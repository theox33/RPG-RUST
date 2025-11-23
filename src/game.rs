mod coffre;
pub mod combat;
mod healthbar;
mod map_utils;
mod movement;
mod rendering;
mod world_mgmt;

use crate::ennemi::Ennemi;
use crate::joueur::Joueur;
use crate::types::{Combatant, Position, PLAYER_BASE_STATS};
use crate::world::World;
use coffre::ChestSystem;
use combat::{CombatInput, CombatResolution, CombatResult, CombatState, CombatTransition};
use healthbar::{HealthBar, HealthBarAnchor};
use macroquad::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use map_utils::{
    default_map_tiles, enemy_cap, load_tiles_for_world, spawn_random_enemies, start_enemy_thread,
};

pub const TILE_SIZE: f32 = 48.0;
pub const MAP_WIDTH: usize = 20;
pub const MAP_HEIGHT: usize = 12;
pub const PLAYER_START_X: usize = 2;
pub const PLAYER_START_Y: usize = 3;
pub const MAX_ENEMIES: usize = 4;
const HOUSE_TILE_WIDTH: f32 = 5.0;
const HOUSE_TILE_HEIGHT: f32 = 4.0;
const HOUSE_FIT_MARGIN: f32 = 0.98;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TileType {
    Herbe,
    Chemin,
    Eau,
    Maison,
    Portal,
    SpiralPortal,
    SpiralPortal2,
    CollisionInvisible,
    Coffre,
    VictoryChest,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
enum WorldKind {
    Plaine,
    Maison,
    Spirale,
    Spirale2,
}

impl WorldKind {
    fn filename(self) -> &'static str {
        match self {
            WorldKind::Plaine => "plaine.map",
            WorldKind::Maison => "maison.map",
            WorldKind::Spirale => "spirale.map",
            WorldKind::Spirale2 => "spirale2.map",
        }
    }

    fn entry_message(self) -> &'static str {
        match self {
            WorldKind::Plaine => "Vous retournez dehors.",
            WorldKind::Maison => "Vous entrez dans la maison.",
            WorldKind::Spirale => "Vous sentez une étrange énergie en spirale...",
            WorldKind::Spirale2 => "Un vortex de puissance s'ouvre devant vous...",
        }
    }

    fn all() -> &'static [WorldKind] {
        use WorldKind::*;
        &[Plaine, Maison, Spirale, Spirale2]
    }
}

pub struct GameTextures {
    pub map_texture: Texture2D,
    pub chemin_src: Rect,
    pub chemin_variants: Vec<Rect>,
    pub house_src: Rect,
    pub maison_background: Texture2D,
    pub char_texture: Texture2D,
    pub char_frames: Vec<Rect>,
    pub grass_variants: Vec<Rect>,
    pub enemy_texture: Texture2D,
    pub enemy_frames: Vec<Rect>,
    pub water_src: Rect,
    pub chest_texture: Texture2D,
    pub chest_frames: Vec<Rect>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Direction {
    Down,
    Right,
    Left,
    Up,
}

impl Direction {
    fn frame_offset(self) -> usize {
        match self {
            Direction::Down => 0,
            Direction::Right => 3,
            Direction::Left => 6,
            Direction::Up => 9,
        }
    }
}

#[derive(Clone)]
struct PlayerAnim {
    direction: Direction,
    frame: usize,
    timer: f32,
    frame_duration: f32,
}

impl PlayerAnim {
    fn new() -> Self {
        Self {
            direction: Direction::Down,
            frame: 0,
            timer: 0.0,
            frame_duration: 0.2,
        }
    }

    fn update(&mut self, dt: f32, moving: bool) {
        if moving {
            self.timer += dt;
            if self.timer > self.frame_duration {
                self.timer -= self.frame_duration;
                self.frame = (self.frame + 1) % 3;
            }
        } else {
            self.frame = 0;
            self.timer = 0.0;
        }
    }

    fn current_frame_index(&self) -> usize {
        self.direction.frame_offset() + self.frame
    }
}

#[derive(Clone)]
struct EnemyAnim {
    moving: bool,
    from: Position,
    to: Position,
    progress: f32,
    frame: usize,
    timer: f32,
    frame_duration: f32,
}

struct Message {
    texte: String,
    timer: f32,
    centered: bool,
}

#[derive(Debug)]
pub struct GameOverState {
    pub selected: usize, // 0 = rejouer, 1 = quitter
    pub allow_restart: bool,
}

enum GameState {
    Exploration,
    Combat(CombatState),
    GameOver(GameOverState),
}

pub struct Game {
    world: Arc<Mutex<World>>,
    state: GameState,
    messages: Vec<Message>,
    textures: GameTextures,
    current_world: WorldKind,
    enemy_cache: HashMap<WorldKind, Vec<Ennemi>>,
    map_tiles: Vec<Vec<TileType>>,
    house_anchors: Vec<Position>,
    chests: ChestSystem,
    player_anim: PlayerAnim,
    moving: bool,
    move_time: f32,
    move_progress: f32,
    move_from: Position,
    move_to: Position,
    grass_choice: Vec<Vec<usize>>,
    chemin_choice: Vec<Vec<usize>>,
    enemy_anim: Vec<EnemyAnim>,
    enemy_prev_positions: Vec<Position>,
    health_bar: HealthBar,
}

impl Game {
    fn check_game_over(&mut self) {
        if let Ok(world) = self.world.lock() {
            if let Some(player) = world.players().get(0) {
                if player.stats().vie <= 0 {
                    self.state = GameState::GameOver(GameOverState {
                        selected: 0,
                        allow_restart: true,
                    });
                }
            }
        }
    }

    fn update_game_over(&mut self) {
        let allow_restart = matches!(
            self.state,
            GameState::GameOver(GameOverState {
                allow_restart: true,
                ..
            })
        );

        if allow_restart && is_key_pressed(KeyCode::Left) {
            if let GameState::GameOver(ref mut state) = self.state {
                state.selected = 0;
            }
        }
        if is_key_pressed(KeyCode::Right) {
            if let GameState::GameOver(ref mut state) = self.state {
                state.selected = 1;
            }
        }
        if !allow_restart {
            if let GameState::GameOver(ref mut state) = self.state {
                state.selected = 1;
            }
        }

        let (screen_w, screen_h) = (screen_width(), screen_height());
        let btn_w = 180.0;
        let btn_h = 48.0;
        let btn_y = screen_h * 0.5 + 40.0;
        let btn1_x = screen_w * 0.5 - btn_w - 20.0;
        let btn2_x = if allow_restart {
            screen_w * 0.5 + 20.0
        } else {
            screen_w * 0.5 - btn_w * 0.5
        };
        let mouse = if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            Some((mx, my))
        } else {
            None
        };
        let mut clicked = None;
        if let Some((mx, my)) = mouse {
            if allow_restart
                && mx >= btn1_x
                && mx <= btn1_x + btn_w
                && my >= btn_y
                && my <= btn_y + btn_h
            {
                clicked = Some(0);
            }
            if mx >= btn2_x && mx <= btn2_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                clicked = Some(1);
            }
        }
        let enter = is_key_pressed(KeyCode::Enter);
        let selected = if let GameState::GameOver(ref state) = self.state {
            state.selected
        } else {
            0
        };
        if enter || clicked.is_some() {
            let action = clicked.unwrap_or(selected);
            match action {
                0 if allow_restart => self.restart_game(),
                1 => std::process::exit(0),
                _ => {}
            }
        }
    }

    fn draw_game_over(&self) {
        let (screen_w, screen_h) = (screen_width(), screen_height());
        let allow_restart = matches!(
            self.state,
            GameState::GameOver(GameOverState {
                allow_restart: true,
                ..
            })
        );
        let selected = if let GameState::GameOver(ref state) = self.state {
            state.selected
        } else {
            0
        };

        if allow_restart {
            let rect_w = 520.0;
            let rect_h = 220.0;
            let rect_x = (screen_w - rect_w) * 0.5;
            let rect_y = (screen_h - rect_h) * 0.5;
            draw_rectangle(rect_x, rect_y, rect_w, rect_h, BLACK);
            let text = "GAME OVER";
            let font_size = 48.0;
            let tw = measure_text(text, None, font_size as u16, 1.0).width;
            draw_text(
                text,
                screen_w * 0.5 - tw * 0.5,
                rect_y + 64.0,
                font_size,
                WHITE,
            );
            let btn_w = 180.0;
            let btn_h = 48.0;
            let btn_y = screen_h * 0.5 + 40.0;
            let btn1_x = screen_w * 0.5 - btn_w - 20.0;
            let btn2_x = screen_w * 0.5 + 20.0;
            draw_rectangle(
                btn1_x,
                btn_y,
                btn_w,
                btn_h,
                if selected == 0 { DARKGRAY } else { GRAY },
            );
            draw_rectangle(
                btn2_x,
                btn_y,
                btn_w,
                btn_h,
                if selected == 1 { DARKGRAY } else { GRAY },
            );
            draw_text("Rejouer", btn1_x + 32.0, btn_y + 32.0, 28.0, WHITE);
            draw_text("Quitter", btn2_x + 32.0, btn_y + 32.0, 28.0, WHITE);
        } else {
            self.draw_messages();
            let btn_w = 200.0;
            let btn_h = 52.0;
            let btn_x = screen_w * 0.5 - btn_w * 0.5;
            let btn_y = screen_h * 0.5 + 40.0;
            draw_rectangle(
                btn_x,
                btn_y,
                btn_w,
                btn_h,
                if selected == 1 { DARKGRAY } else { GRAY },
            );
            draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 2.0, WHITE);
            draw_text("Quitter", btn_x + 48.0, btn_y + 34.0, 28.0, WHITE);
        }
    }

    fn restart_game(&mut self) {
        // Recharger la map et ses variantes
        let mut map_tiles = load_tiles_for_world(WorldKind::Plaine);
        if map_tiles.len() != MAP_HEIGHT || map_tiles[0].len() != MAP_WIDTH {
            map_tiles = default_map_tiles();
        }
        self.map_tiles = map_tiles;
        self.current_world = WorldKind::Plaine;
        self.refresh_map_variants();
        self.rebuild_chests_from_tiles();
        self.sync_world_walkable_map();

        // Réinitialiser le joueur
        if let Ok(mut world) = self.world.lock() {
            world.players.clear();
            world.add_player(Joueur::nouveau(
                0,
                Position {
                    x: PLAYER_START_X,
                    y: PLAYER_START_Y,
                },
            ));
            // Réinitialiser les ennemis
            world.enemies.clear();
            spawn_random_enemies(
                &mut world,
                &self.map_tiles,
                enemy_cap(self.current_world),
                self.current_world,
            );
            world.enemies_frozen = false;
        }

        // Réinitialiser l’animation du joueur
        self.player_anim = PlayerAnim::new();
        self.moving = false;
        self.move_progress = 0.0;
        self.move_from = Position {
            x: PLAYER_START_X,
            y: PLAYER_START_Y,
        };
        self.move_to = Position {
            x: PLAYER_START_X,
            y: PLAYER_START_Y,
        };

        // Réinitialiser l’animation des ennemis
        let mut positions = Vec::new();
        if let Ok(world) = self.world.lock() {
            positions = world.enemies().iter().map(|e| e.position()).collect();
        }
        self.rebuild_enemy_animation_state(positions);

        // Réinitialiser les coffres
        self.chests = ChestSystem::new();
        self.rebuild_chests_from_tiles();

        self.state = GameState::Exploration;
        self.messages.clear();
    }
    pub fn new(
        map_texture: Texture2D,
        char_texture: Texture2D,
        enemy_texture: Texture2D,
        maison_background: Texture2D,
        chest_texture: Texture2D,
    ) -> Self {
        let chemin_src = Rect::new(300.0, 20.0, 100.0, 100.0);
        let chemin_variants = vec![
            Rect::new(450.0, 20.0, 100.0, 100.0),
            Rect::new(595.0, 20.0, 100.0, 100.0),
            Rect::new(740.0, 20.0, 100.0, 100.0),
        ];
        let water_src = Rect::new(170.0, 170.0, 100.0, 100.0);
        let house_src = Rect::new(640.0, 768.0, 320.0, 256.0);
        let grass_variants = vec![
            Rect::new(20.0, 20.0, 100.0, 100.0),
            Rect::new(170.0, 20.0, 100.0, 100.0),
            Rect::new(20.0, 170.0, 100.0, 100.0),
            Rect::new(450.0, 170.0, 100.0, 100.0),
            Rect::new(450.0, 320.0, 100.0, 100.0),
        ];
        let cols = 3;
        let rows = 4;
        let cw = char_texture.width() / cols as f32;
        let ch = char_texture.height() / rows as f32;
        let offset_y = 2.0;
        let mut char_frames = Vec::new();
        for row in 0..rows {
            for col in 0..cols {
                char_frames.push(Rect::new(
                    col as f32 * cw,
                    row as f32 * ch + offset_y,
                    cw,
                    ch - offset_y,
                ));
            }
        }
        let e_cols = 2;
        let e_rows = 3;
        let ecw = enemy_texture.width() / e_cols as f32;
        let ech = enemy_texture.height() / e_rows as f32;
        let mut enemy_frames = Vec::new();
        for row in 0..e_rows {
            enemy_frames.push(Rect::new(0.0, row as f32 * ech, ecw, ech));
        }
        for row in (0..e_rows).rev() {
            enemy_frames.push(Rect::new(ecw, row as f32 * ech, ecw, ech));
        }

        let chest_cols = 3;
        let chest_rows = 2;
        let tex_w = chest_texture.width();
        let tex_h = chest_texture.height();
        let mut chest_frames = Vec::new();
        for row in 0..chest_rows {
            for col in 0..chest_cols {
                let x0 = tex_w * col as f32 / chest_cols as f32;
                let x1 = tex_w * (col as f32 + 1.0) / chest_cols as f32;
                let y0 = tex_h * row as f32 / chest_rows as f32;
                let y1 = tex_h * (row as f32 + 1.0) / chest_rows as f32;
                chest_frames.push(Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)));
            }
        }
        let textures = GameTextures {
            map_texture,
            chemin_src,
            chemin_variants,
            house_src,
            maison_background,
            char_texture,
            char_frames,
            grass_variants,
            enemy_texture,
            enemy_frames,
            water_src,
            chest_texture,
            chest_frames,
        };

        let mut map_tiles = load_tiles_for_world(WorldKind::Plaine);
        if map_tiles.len() != MAP_HEIGHT || map_tiles[0].len() != MAP_WIDTH {
            map_tiles = default_map_tiles();
        }

        let mut world_data = World::new(MAP_WIDTH, MAP_HEIGHT);
        world_data.add_player(Joueur::nouveau(
            0,
            Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
        ));

        let world = Arc::new(Mutex::new(world_data));
        let mut game = Self {
            world: Arc::clone(&world),
            state: GameState::Exploration,
            messages: Vec::new(),
            textures,
            current_world: WorldKind::Plaine,
            enemy_cache: HashMap::new(),
            map_tiles,
            house_anchors: Vec::new(),
            chests: ChestSystem::new(),
            player_anim: PlayerAnim::new(),
            moving: false,
            move_time: 0.3,
            move_progress: 0.0,
            move_from: Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
            move_to: Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
            grass_choice: vec![vec![0; MAP_WIDTH]; MAP_HEIGHT],
            chemin_choice: vec![vec![0; MAP_WIDTH]; MAP_HEIGHT],
            enemy_anim: Vec::new(),
            enemy_prev_positions: Vec::new(),
            health_bar: HealthBar::new(
                260.0,
                28.0,
                20.0,
                PLAYER_BASE_STATS.vie,
                HealthBarAnchor::TopLeft,
            ),
        };

        game.refresh_map_variants();
        game.rebuild_chests_from_tiles();
        game.sync_world_walkable_map();
        game.preload_enemy_caches();
        game.respawn_enemies_for_current_world();
        start_enemy_thread(&game.world);
        game
    }

    pub fn frame(&mut self) {
        clear_background(LIGHTGRAY);
        let dt = get_frame_time();
        self.update_messages(dt);
        self.check_game_over();
        match &mut self.state {
            GameState::Exploration => {
                self.update_movement(dt);
                self.update_enemy_movement(dt);
                self.update_exploration(dt);
                self.update_chest_prompt();
                self.handle_chest_ui_input();
            }
            GameState::Combat(_) => {
                self.update_combat_state();
                self.chests.clear_active();
            }
            GameState::GameOver(_) => {
                self.update_game_over();
            }
        }
        self.update_chest_animation(dt);
        self.render();
    }

    fn update_messages(&mut self, dt: f32) {
        for msg in &mut self.messages {
            if msg.timer > dt {
                msg.timer -= dt;
            } else {
                msg.timer = 0.0;
            }
        }
        self.messages.retain(|msg| msg.timer > 0.0);
    }

    fn update_chest_animation(&mut self, dt: f32) {
        self.chests
            .update_animation(dt, self.textures.chest_frames.len());
        let rewards = self.chests.collect_opened_rewards();
        for position in rewards {
            if matches!(
                self.map_tiles[position.y][position.x],
                TileType::VictoryChest
            ) {
                self.trigger_victory();
            } else {
                self.apply_player_attack_bonus();
            }
        }
    }

    fn update_chest_prompt(&mut self) {
        if !matches!(self.state, GameState::Exploration) {
            self.chests.clear_active();
            return;
        }
        let player_pos = match self.world.lock() {
            Ok(world) => world.players().get(0).map(|p| p.position()),
            Err(_) => None,
        };
        self.chests.refresh_prompt(player_pos);
    }

    fn handle_chest_ui_input(&mut self) {
        if !matches!(self.state, GameState::Exploration) {
            self.chests.clear_active();
            return;
        }
        if !is_mouse_button_pressed(MouseButton::Left) {
            return;
        }
        let (mx, my) = mouse_position();
        if let Some(position) = self.chests.handle_click(Vec2::new(mx, my)) {
            let tile = self.map_tiles[position.y][position.x];
            let (texte, timer, centered) = if tile == TileType::VictoryChest {
                (
                    String::from(
                        "Bravo, vous avez vaincu tous les ennemis! Le monde est plus sûr grâce à vous!",
                    ),
                    3.0,
                    true,
                )
            } else {
                (String::from("Vous ouvrez le coffre."), 1.2, false)
            };
            self.messages.push(Message {
                texte,
                timer,
                centered,
            });
            if tile == TileType::VictoryChest {
                self.trigger_victory();
            }
        }
    }

    fn tile_walkable(&self, x: usize, y: usize) -> bool {
        matches!(
            self.map_tiles[y][x],
            TileType::Herbe
                | TileType::Chemin
                | TileType::Portal
                | TileType::SpiralPortal
                | TileType::SpiralPortal2
        ) && !matches!(self.map_tiles[y][x], TileType::CollisionInvisible)
    }

    fn remaining_enemies_total(&self) -> usize {
        let mut total = 0;
        if let Ok(world) = self.world.lock() {
            total += world
                .enemies()
                .iter()
                .filter(|enemy| enemy.est_vivant())
                .count();
        }
        for (kind, enemies) in &self.enemy_cache {
            if *kind == self.current_world {
                continue;
            }
            total += enemies.iter().filter(|enemy| enemy.est_vivant()).count();
        }
        total
    }
}
