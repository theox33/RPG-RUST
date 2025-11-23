pub mod combat;
mod healthbar;
mod coffre;

use crate::ennemi::Ennemi;
use crate::joueur::Joueur;
use crate::types::{Combatant, Position, PLAYER_BASE_STATS};
use crate::world::World;
use ::rand::{thread_rng, Rng};
use combat::{CombatInput, CombatResolution, CombatResult, CombatState, CombatTransition};
use healthbar::{HealthBar, HealthBarAnchor};
use macroquad::prelude::*;
use coffre::ChestSystem;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
                        self.state = GameState::GameOver(GameOverState { selected: 0 });
                    }
                }
            }
        }

        fn update_game_over(&mut self) {
            // Flèches gauche/droite
            if is_key_pressed(KeyCode::Left) {
                if let GameState::GameOver(ref mut state) = self.state {
                    state.selected = 0;
                }
            }
            if is_key_pressed(KeyCode::Right) {
                if let GameState::GameOver(ref mut state) = self.state {
                    state.selected = 1;
                }
            }
            // Entrée ou clic
            let (screen_w, screen_h) = (screen_width(), screen_height());
            let btn_w = 180.0;
            let btn_h = 48.0;
            let btn_y = screen_h * 0.5 + 40.0;
            let btn1_x = screen_w * 0.5 - btn_w - 20.0;
            let btn2_x = screen_w * 0.5 + 20.0;
            let mouse = if is_mouse_button_pressed(MouseButton::Left) {
                let (mx, my) = mouse_position();
                Some((mx, my))
            } else { None };
            let mut clicked = None;
            if let Some((mx, my)) = mouse {
                if mx >= btn1_x && mx <= btn1_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                    clicked = Some(0);
                }
                if mx >= btn2_x && mx <= btn2_x + btn_w && my >= btn_y && my <= btn_y + btn_h {
                    clicked = Some(1);
                }
            }
            let enter = is_key_pressed(KeyCode::Enter);
            let selected = if let GameState::GameOver(ref state) = self.state { state.selected } else { 0 };
            if enter || clicked.is_some() {
                let action = clicked.unwrap_or(selected);
                match action {
                    0 => self.restart_game(),
                    1 => std::process::exit(0),
                    _ => {}
                }
            }
        }

        fn draw_game_over(&self) {
            let (screen_w, screen_h) = (screen_width(), screen_height());
            let rect_w = 520.0;
            let rect_h = 220.0;
            let rect_x = (screen_w - rect_w) * 0.5;
            let rect_y = (screen_h - rect_h) * 0.5;
            draw_rectangle(rect_x, rect_y, rect_w, rect_h, BLACK);
            let text = "GAME OVER";
            let font_size = 48.0;
            let tw = measure_text(text, None, font_size as u16, 1.0).width;
            draw_text(text, screen_w * 0.5 - tw * 0.5, rect_y + 64.0, font_size, WHITE);
            let btn_w = 180.0;
            let btn_h = 48.0;
            let btn_y = screen_h * 0.5 + 40.0;
            let btn1_x = screen_w * 0.5 - btn_w - 20.0;
            let btn2_x = screen_w * 0.5 + 20.0;
            let selected = if let GameState::GameOver(ref state) = self.state { state.selected } else { 0 };
            draw_rectangle(btn1_x, btn_y, btn_w, btn_h, if selected == 0 { DARKGRAY } else { GRAY });
            draw_rectangle(btn2_x, btn_y, btn_w, btn_h, if selected == 1 { DARKGRAY } else { GRAY });
            draw_text("Rejouer", btn1_x + 32.0, btn_y + 32.0, 28.0, WHITE);
            draw_text("Quitter", btn2_x + 32.0, btn_y + 32.0, 28.0, WHITE);
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
            health_bar: HealthBar::new(260.0, 28.0, 20.0, PLAYER_BASE_STATS.vie, HealthBarAnchor::TopLeft),
        };

        game.refresh_map_variants();
        game.rebuild_chests_from_tiles();
        game.sync_world_walkable_map();
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

    fn update_movement(&mut self, dt: f32) {
        if self.moving {
            self.move_progress += dt / self.move_time;
            if self.move_progress >= 1.0 {
                self.moving = false;
                self.move_progress = 0.0;
            }
        }
    }

    fn update_enemy_movement(&mut self, dt: f32) {
        let (positions, player_pos) = match self.world.lock() {
            Ok(world) => (
                world
                    .enemies()
                    .iter()
                    .map(|e| e.position())
                    .collect::<Vec<_>>(),
                world.players().get(0).map(|p| p.position()),
            ),
            Err(_) => return,
        };
        self.enemy_anim.truncate(positions.len());
        self.enemy_prev_positions.truncate(positions.len());
        while self.enemy_anim.len() < positions.len() {
            let pos = positions[self.enemy_anim.len()];
            self.enemy_anim.push(EnemyAnim {
                moving: false,
                from: pos,
                to: pos,
                progress: 0.0,
                frame: 0,
                timer: 0.0,
                frame_duration: 0.0,
            });
        }
        while self.enemy_prev_positions.len() < positions.len() {
            self.enemy_prev_positions
                .push(positions[self.enemy_prev_positions.len()]);
        }
        for (i, pos) in positions.iter().enumerate() {
            let prev = self.enemy_prev_positions[i];
            let anim = &mut self.enemy_anim[i];
            if anim.moving {
                anim.progress += dt / self.move_time;
                anim.timer += dt;
                if anim.frame_duration > 0.0 && anim.timer > anim.frame_duration {
                    anim.timer -= anim.frame_duration;
                    anim.frame = (anim.frame + 1) % self.textures.enemy_frames.len();
                }
                if anim.progress >= 1.0 {
                    anim.moving = false;
                    anim.progress = 0.0;
                    anim.frame = 0;
                    anim.timer = 0.0;
                    self.enemy_prev_positions[i] = anim.to;
                }
            } else if pos.x != prev.x || pos.y != prev.y {
                let (nx, ny) = (pos.x, pos.y);
                let blocked = nx < MAP_WIDTH
                    && ny < MAP_HEIGHT
                    && !matches!(
                        self.map_tiles[ny][nx],
                        TileType::Herbe | TileType::Chemin | TileType::Portal
                    );
                if blocked {
                    if let Ok(mut world) = self.world.lock() {
                        if let Some(enemy) = world.enemies_mut().get_mut(i) {
                            let epos = enemy.position_mut();
                            epos.x = prev.x;
                            epos.y = prev.y;
                        }
                    }
                    self.enemy_prev_positions[i] = prev;
                } else {
                    anim.moving = true;
                    anim.from = prev;
                    anim.to = *pos;
                    anim.progress = 0.0;
                    anim.frame = 0;
                    anim.timer = 0.0;
                    anim.frame_duration = self.move_time / self.textures.enemy_frames.len() as f32;
                    self.enemy_prev_positions[i] = *pos;
                }
            }
        }

        if matches!(self.state, GameState::Exploration) {
            if let Some(player_pos) = player_pos {
                if let Some((enemy_idx, _)) = positions
                    .iter()
                    .enumerate()
                    .find(|(_, pos)| pos.x == player_pos.x && pos.y == player_pos.y)
                {
                    self.engage_combat_with_enemy(enemy_idx, "Un ennemi vous attaque !");
                }
            }
        }
    }

    fn update_exploration(&mut self, dt: f32) {
        if self.moving {
            self.player_anim.update(dt, true);
            return;
        }

        let mut dx: isize = 0;
        let mut dy: isize = 0;
        if is_key_down(KeyCode::Up) || is_key_down(KeyCode::Z) {
            dy = -1;
        } else if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
            dy = 1;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            dx = 1;
        } else if is_key_down(KeyCode::Left) || is_key_down(KeyCode::Q) {
            dx = -1;
        }

        let moving_input = dx != 0 || dy != 0;
        if moving_input {
            if is_key_down(KeyCode::Up) || is_key_down(KeyCode::Z) {
                self.player_anim.direction = Direction::Up;
            } else if is_key_down(KeyCode::Down) || is_key_down(KeyCode::S) {
                self.player_anim.direction = Direction::Down;
            }
            if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
                self.player_anim.direction = Direction::Right;
            } else if is_key_down(KeyCode::Left) || is_key_down(KeyCode::Q) {
                self.player_anim.direction = Direction::Left;
            }
        }
        self.player_anim.update(dt, moving_input);
        if !moving_input {
            return;
        }

        let mut world = match self.world.lock() {
            Ok(world) => world,
            Err(_) => return,
        };
        if world.players().is_empty() {
            return;
        }
        let old_pos = world.players()[0].position();
        let nx = old_pos.x as isize + dx;
        let ny = old_pos.y as isize + dy;
        if nx < 0 || ny < 0 || nx >= MAP_WIDTH as isize || ny >= MAP_HEIGHT as isize {
            return;
        }
        let (nxu, nyu) = (nx as usize, ny as usize);
        if !self.tile_walkable(nxu, nyu) {
            return;
        }
        if let Some(e_idx) = world.find_enemy_on_tile(nxu, nyu) {
            drop(world);
            self.engage_combat_with_enemy(
                e_idx,
                "Vous tentez d'entrer sur la case d'un ennemi : combat engagé !",
            );
        } else if world.move_player(0, dx, dy) {
            let new_pos = world.players()[0].position();
            drop(world);
            self.begin_move_animation(old_pos, new_pos);
            self.messages.push(Message {
                texte: String::from("Vous vous déplacez."),
                timer: 0.6,
                centered: false,
            });
            match self.map_tiles[new_pos.y][new_pos.x] {
                TileType::Portal => {
                    self.trigger_portal_transition();
                    return;
                }
                TileType::SpiralPortal => {
                    self.trigger_spiral_transition();
                    return;
                }
                TileType::SpiralPortal2 => {
                    self.trigger_spiral2_transition();
                    return;
                }
                _ => {}
            }
        }
    }

    fn begin_move_animation(&mut self, from: Position, to: Position) {
        self.moving = true;
        self.move_progress = 0.0;
        self.move_from = from;
        self.move_to = to;
        self.player_anim.frame = 0;
        self.player_anim.timer = 0.0;
        let nframes = 3;
        self.player_anim.frame_duration = self.move_time / nframes as f32;
    }

    fn engage_combat_with_enemy(&mut self, enemy_idx: usize, message: &str) {
        if !matches!(self.state, GameState::Exploration) {
            return;
        }
        let mut world = match self.world.lock() {
            Ok(world) => world,
            Err(_) => return,
        };
        if world.players().is_empty() || enemy_idx >= world.enemies().len() {
            return;
        }
        self.messages.push(Message {
            texte: message.to_string(),
            timer: 1.2,
            centered: false,
        });
        world.enemies_frozen = true;
        let player_speed = world.players()[0].vitesse();
        let enemy_speed = world.enemies()[enemy_idx].vitesse();
        let player_first = player_speed >= enemy_speed;
        self.state = GameState::Combat(CombatState::with_initiative(0, enemy_idx, player_first));
    }

    fn update_combat_state(&mut self) {
        let current = std::mem::replace(&mut self.state, GameState::Exploration);
        if let GameState::Combat(mut state) = current {
            let still_fighting = self.update_combat(&mut state);
            if still_fighting {
                self.state = GameState::Combat(state);
            } else if let Ok(mut world) = self.world.lock() {
                world.enemies_frozen = false;
            }
        } else {
            self.state = current;
        }
    }

    fn update_combat(&mut self, state: &mut CombatState) -> bool {
        let keys = self.collect_combat_keys();
        let mouse_click = if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            Some(Vec2::new(mx, my))
        } else {
            None
        };
        let mut world = match self.world.lock() {
            Ok(world) => world,
            Err(_) => return false,
        };
        let (origin_x, origin_y) = self.world_origin();
        let input = CombatInput {
            keys_pressed: keys,
            mouse_click,
            tile_size: TILE_SIZE,
            world_height: world.height,
        };
        let CombatResolution {
            messages,
            transition,
        } = state.update(&mut world, &input, origin_x, origin_y);
        for msg in messages {
            self.messages.push(Message {
                texte: msg.texte,
                timer: msg.duree,
                centered: false,
            });
        }
        let mut rebuild_positions: Option<Vec<Position>> = None;
        let continue_fight = match transition {
            CombatTransition::Continue => true,
            CombatTransition::Terminer(result) => {
                if matches!(result, CombatResult::JoueurVainqueur | CombatResult::DoubleKo) {
                    let enemy_idx = state.enemy_index();
                    if enemy_idx < world.enemies.len() {
                        world.enemies.remove(enemy_idx);
                        rebuild_positions = Some(
                            world
                                .enemies()
                                .iter()
                                .map(|e| e.position())
                                .collect(),
                        );
                    }
                }
                if let CombatResult::EnnemiVainqueur | CombatResult::DoubleKo = result {
                    self.messages.push(Message {
                        texte: String::from("Vous êtes vaincu..."),
                        timer: 2.0,
                        centered: false,
                    });
                }
                false
            }
        };
        drop(world);
        if let Some(positions) = rebuild_positions {
            self.rebuild_enemy_animation_state(positions);
        }
        continue_fight
    }

    fn collect_combat_keys(&self) -> Vec<KeyCode> {
        [KeyCode::A, KeyCode::D, KeyCode::F]
            .iter()
            .copied()
            .filter(|key| is_key_pressed(*key))
            .collect()
    }

    fn render(&mut self) {
        let (origin_x, origin_y) = self.world_origin();
        self.draw_tiles(origin_x, origin_y);
        self.chests.draw(&self.textures, origin_x, origin_y);
        if matches!(self.current_world, WorldKind::Plaine) {
            self.draw_houses(origin_x, origin_y);
        }
        if let Ok(world) = self.world.lock() {
            match &self.state {
                GameState::GameOver(_) => {
                    // Ne pas dessiner le joueur ni les ennemis
                }
                GameState::Combat(state) => {
                    self.draw_player(&world, origin_x, origin_y);
                    self.draw_enemies(&world, origin_x, origin_y);
                    // Barre de vie au-dessus de l'ennemi courant
                    let enemy = &world.enemies()[state.enemy_index()];
                    let ex = origin_x + enemy.position().x as f32 * TILE_SIZE;
                    let ey = origin_y + enemy.position().y as f32 * TILE_SIZE;
                    let bar_w = 48.0;
                    let bar_h = 8.0;
                    let bar_x = ex + (TILE_SIZE - bar_w) * 0.5;
                    let bar_y = ey - bar_h - 6.0;
                    // Vie max de l'ennemi = 100
                    let max_vie = 100;
                    let ratio = (enemy.stats().vie as f32 / max_vie as f32).clamp(0.0, 1.0);
                    let filled = bar_w * ratio;
                    let bg = Color::new(0.1, 0.1, 0.1, 0.85);
                    draw_rectangle(bar_x, bar_y, bar_w, bar_h, bg);
                    draw_rectangle_lines(bar_x, bar_y, bar_w, bar_h, 1.5, WHITE);
                    draw_rectangle(bar_x, bar_y, filled, bar_h, RED);
                    state.draw_ui(&world, TILE_SIZE, origin_x, origin_y);
                }
                _ => {
                    self.draw_player(&world, origin_x, origin_y);
                    self.draw_enemies(&world, origin_x, origin_y);
                }
            }
        }
        if let GameState::GameOver(_) = self.state {
            self.draw_game_over();
        } else {
            self.draw_messages();
            self.chests.draw_prompt();
            self.draw_health_bar();
        }
    }

    fn draw_health_bar(&self) {
        let (origin_x, origin_y) = self.world_origin();
        if let Ok(world) = self.world.lock() {
            if let Some(player) = world.players().get(0) {
                self.health_bar.draw_at(player.stats(), origin_x, origin_y);
            }
        }
    }

    fn world_origin(&self) -> (f32, f32) {
        let world_w = MAP_WIDTH as f32 * TILE_SIZE;
        let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
        let origin_x = ((screen_width() - world_w) * 0.5).max(0.0);
        let origin_y = ((screen_height() - world_h) * 0.5).max(0.0);
        (origin_x, origin_y)
    }

    fn draw_tiles(&self, origin_x: f32, origin_y: f32) {
        if matches!(self.current_world, WorldKind::Maison) {
            self.draw_maison_background(origin_x, origin_y);
        }
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = self.map_tiles[y][x];
                if matches!(self.current_world, WorldKind::Maison)
                    && !matches!(tile, TileType::Portal)
                {
                    continue;
                }
                let source = match tile {
                    TileType::Herbe => {
                        let idx = self.grass_choice[y][x]
                            .min(self.textures.grass_variants.len().saturating_sub(1));
                        self.textures.grass_variants[idx]
                    }
                    TileType::Chemin => {
                        if self.textures.chemin_variants.is_empty() {
                            self.textures.chemin_src
                        } else {
                            let idx = self.chemin_choice[y][x]
                                .min(self.textures.chemin_variants.len() - 1);
                            self.textures.chemin_variants[idx]
                        }
                    }
                    TileType::Eau => self.textures.water_src,
                    TileType::Maison => self
                        .textures
                        .grass_variants
                        .get(0)
                        .copied()
                        .unwrap_or(self.textures.chemin_src),
                    TileType::Portal | TileType::SpiralPortal | TileType::SpiralPortal2 => {
                        // Affiche comme un chemin (variation)
                        if !self.textures.chemin_variants.is_empty() {
                            self.textures.chemin_variants[0]
                        } else {
                            self.textures.chemin_src
                        }
                    }
                    TileType::CollisionInvisible | TileType::Coffre => {
                        // Texture neutre ou rien
                        Rect::new(0.0, 0.0, 0.0, 0.0)
                    },
                };
                let dest_x = origin_x + x as f32 * TILE_SIZE;
                let dest_y = origin_y + y as f32 * TILE_SIZE;
                draw_texture_ex(
                    &self.textures.map_texture,
                    dest_x,
                    dest_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::splat(TILE_SIZE)),
                        source: Some(source),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn update_chest_animation(&mut self, dt: f32) {
        self.chests
            .update_animation(dt, self.textures.chest_frames.len());
        let rewards = self.chests.collect_opened_rewards();
        for _ in rewards {
            self.apply_player_attack_bonus();
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
        if self.chests.handle_click(Vec2::new(mx, my)) {
            self.messages.push(Message {
                texte: String::from("Vous ouvrez le coffre."),
                timer: 1.2,
                centered: false,
            });
        }
    }

    fn draw_maison_background(&self, origin_x: f32, origin_y: f32) {
        let world_w = MAP_WIDTH as f32 * TILE_SIZE;
        let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
        draw_texture_ex(
            &self.textures.maison_background,
            origin_x,
            origin_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(world_w, world_h)),
                ..Default::default()
            },
        );
    }

    fn draw_player(&self, world: &World, origin_x: f32, origin_y: f32) {
        if world.players().is_empty() {
            return;
        }
        let tile_pos = if self.moving {
            let t = self.move_progress.clamp(0.0, 1.0);
            let lerp_x =
                self.move_from.x as f32 + (self.move_to.x as f32 - self.move_from.x as f32) * t;
            let lerp_y =
                self.move_from.y as f32 + (self.move_to.y as f32 - self.move_from.y as f32) * t;
            (lerp_x, lerp_y)
        } else {
            let pos = world.players()[0].position();
            (pos.x as f32, pos.y as f32)
        };
        let frame = self.player_anim.current_frame_index();
        let frame = frame % self.textures.char_frames.len();
        let src = self.textures.char_frames[frame];
        let dest_x = origin_x + tile_pos.0 * TILE_SIZE;
        let dest_y = origin_y + tile_pos.1 * TILE_SIZE - 6.0;
        draw_texture_ex(
            &self.textures.char_texture,
            dest_x,
            dest_y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE * 1.1)),
                source: Some(src),
                ..Default::default()
            },
        );
    }

    fn draw_enemies(&self, world: &World, origin_x: f32, origin_y: f32) {
        for (idx, enemy) in world.enemies().iter().enumerate() {
            if idx >= self.enemy_anim.len() || !enemy.est_vivant() {
                continue;
            }
            let anim = &self.enemy_anim[idx];
            let tile_pos = if anim.moving {
                let t = anim.progress.clamp(0.0, 1.0);
                let lerp_x = anim.from.x as f32 + (anim.to.x as f32 - anim.from.x as f32) * t;
                let lerp_y = anim.from.y as f32 + (anim.to.y as f32 - anim.from.y as f32) * t;
                (lerp_x, lerp_y)
            } else {
                let pos = enemy.position();
                (pos.x as f32, pos.y as f32)
            };
            let frame = anim.frame % self.textures.enemy_frames.len();
            let src = self.textures.enemy_frames[frame];
            let dest_x = origin_x + tile_pos.0 * TILE_SIZE;
            let dest_y = origin_y + tile_pos.1 * TILE_SIZE - 6.0;
            draw_texture_ex(
                &self.textures.enemy_texture,
                dest_x,
                dest_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE * 1.05)),
                    source: Some(src),
                    ..Default::default()
                },
            );
        }
    }

    fn draw_messages(&self) {
        let mut y = 30.0;
        for msg in self.messages.iter().filter(|m| !m.centered) {
            draw_text(&msg.texte, 20.0, y, 22.0, DARKGRAY);
            y += 24.0;
        }

        let centered: Vec<&Message> = self.messages.iter().filter(|m| m.centered).collect();
        let total = centered.len();
        if total > 0 {
            let stack_spacing = 16.0;
            let box_height = 64.0;
            for (idx, msg) in centered.into_iter().enumerate() {
                let offset = (idx as f32 - (total as f32 - 1.0) * 0.5) * (box_height + stack_spacing);
                self.draw_centered_message(&msg.texte, offset);
            }
        }
    }

    fn draw_centered_message(&self, texte: &str, offset_y: f32) {
        let font_size: u16 = 32;
        let dims = measure_text(texte, None, font_size, 1.0);
        let padding_x = 36.0;
        let padding_y = 22.0;
        let box_w = dims.width + padding_x * 2.0;
        let box_h = dims.height + padding_y * 2.0;
        let center_x = screen_width() * 0.5;
        let center_y = screen_height() * 0.5 + offset_y;
        let rect_x = center_x - box_w * 0.5;
        let rect_y = center_y - box_h * 0.5;
        draw_rectangle(rect_x, rect_y, box_w, box_h, Color::new(0.0, 0.0, 0.0, 0.9));
        draw_rectangle_lines(rect_x, rect_y, box_w, box_h, 2.0, WHITE);
        let text_x = center_x - dims.width * 0.5;
        let text_y = center_y + dims.height * 0.5 - dims.offset_y;
        draw_text(texte, text_x, text_y, font_size as f32, WHITE);
    }

    fn draw_houses(&self, origin_x: f32, origin_y: f32) {
        let max_width = TILE_SIZE * HOUSE_TILE_WIDTH;
        let max_height = TILE_SIZE * HOUSE_TILE_HEIGHT;
        for house in &self.house_anchors {
            let src = self.textures.house_src;
            let scale_w = max_width / src.w;
            let scale_h = max_height / src.h;
            let scale = scale_w.min(scale_h) * HOUSE_FIT_MARGIN;
            let dest_w = src.w * scale;
            let dest_h = src.h * scale;
            let offset_x = (max_width - dest_w) * 0.5;
            let offset_y = max_height - dest_h;
            let dest_x = origin_x + house.x as f32 * TILE_SIZE + offset_x;
            let dest_y = origin_y + house.y as f32 * TILE_SIZE + offset_y;
            draw_texture_ex(
                &self.textures.map_texture,
                dest_x,
                dest_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(Vec2::new(dest_w, dest_h)),
                    source: Some(src),
                    ..Default::default()
                },
            );
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

    fn refresh_map_variants(&mut self) {
        self.grass_choice = choose_grass_variants(&self.textures, &self.map_tiles);
        self.chemin_choice = choose_chemin_variants(&self.textures, &self.map_tiles);
        self.house_anchors = detect_house_anchors(&self.map_tiles);
    }

    fn rebuild_chests_from_tiles(&mut self) {
        self.chests
            .rebuild_from_tiles(&self.map_tiles, self.current_world);
    }

    fn sync_world_walkable_map(&self) {
        if let Ok(mut world) = self.world.lock() {
            world.update_walkable_map(build_walkable_map(&self.map_tiles));
        }
    }

    fn respawn_enemies_for_current_world(&mut self) {
        let mut snapshot = Vec::new();
        if let Ok(mut world) = self.world.lock() {
            world.enemies.clear();
            if let Some(stored) = self.enemy_cache.get(&self.current_world).cloned() {
                snapshot = stored.iter().map(|e| e.position()).collect();
                world.enemies = stored;
            } else {
                let cap = enemy_cap(self.current_world);
                if cap > 0 {
                    spawn_random_enemies(
                        &mut world,
                        &self.map_tiles,
                        cap,
                        self.current_world,
                    );
                }
                snapshot = world
                    .enemies()
                    .iter()
                    .map(|e| e.position())
                    .collect();
            }
        }
        self.rebuild_enemy_animation_state(snapshot);
    }

    fn store_current_world_enemies(&mut self) {
        if let Ok(world) = self.world.lock() {
            self.enemy_cache
                .insert(self.current_world, world.enemies().to_vec());
        }
    }

    fn rebuild_enemy_animation_state(&mut self, positions: Vec<Position>) {
        self.enemy_prev_positions = positions.clone();
        self.enemy_anim = positions
            .into_iter()
            .map(|pos| EnemyAnim {
                moving: false,
                from: pos,
                to: pos,
                progress: 0.0,
                frame: 0,
                timer: 0.0,
                frame_duration: 0.0,
            })
            .collect();
    }

    fn move_player_to(&mut self, pos: Position) {
        if let Ok(mut world) = self.world.lock() {
            if let Some(player) = world.players_mut().get_mut(0) {
                let player_pos = player.position_mut();
                player_pos.x = pos.x;
                player_pos.y = pos.y;
            }
        }
        self.move_from = pos;
        self.move_to = pos;
        self.moving = false;
        self.move_progress = 0.0;
        self.player_anim.frame = 0;
        self.player_anim.timer = 0.0;
    }

    fn trigger_portal_transition(&mut self) {
        let target = match self.current_world {
            WorldKind::Plaine => WorldKind::Maison,
            WorldKind::Maison => WorldKind::Plaine,
            _ => return,
        };
        if self.switch_to_world(target, TileType::Portal) {
            self.messages.push(Message {
                texte: target.entry_message().to_string(),
                timer: 1.2,
                centered: false,
            });
        }
    }

    fn trigger_spiral_transition(&mut self) {
        let target = match self.current_world {
            WorldKind::Plaine => WorldKind::Spirale,
            WorldKind::Spirale => WorldKind::Plaine,
            _ => return,
        };
        if self.switch_to_world(target, TileType::SpiralPortal) {
            self.messages.push(Message {
                texte: target.entry_message().to_string(),
                timer: 1.2,
                centered: false,
            });
        }
    }

    fn trigger_spiral2_transition(&mut self) {
        let target = match self.current_world {
            WorldKind::Plaine => WorldKind::Spirale2,
            WorldKind::Spirale2 => WorldKind::Plaine,
            _ => return,
        };
        if self.switch_to_world(target, TileType::SpiralPortal2) {
            self.messages.push(Message {
                texte: target.entry_message().to_string(),
                timer: 1.2,
                centered: false,
            });
        }
    }

    fn switch_to_world(&mut self, target: WorldKind, spawn_tile: TileType) -> bool {
        let previous_world = self.current_world;
        self.store_current_world_enemies();
        self.store_current_world_chests();
        let mut map_tiles = load_tiles_for_world(target);
        if map_tiles.len() != MAP_HEIGHT || map_tiles[0].len() != MAP_WIDTH {
            map_tiles = default_map_tiles();
        }
        self.map_tiles = map_tiles;
        self.current_world = target;
        self.refresh_map_variants();
        self.rebuild_chests_from_tiles();
        self.sync_world_walkable_map();
        self.respawn_enemies_for_current_world();
        let mut spawn = find_tile_position(&self.map_tiles, spawn_tile).unwrap_or(Position {
            x: PLAYER_START_X,
            y: PLAYER_START_Y,
        });
        // Si transition plaine <-> spirale, garder y et forcer x
        if let Ok(world) = self.world.lock() {
            if let Some(player) = world.players().get(0) {
                let y = player.position().y;
                match (previous_world, target) {
                    (WorldKind::Plaine, WorldKind::Spirale) => {
                        spawn.x = 0;
                        spawn.y = y;
                    }
                    (WorldKind::Spirale, WorldKind::Plaine) => {
                        spawn.x = MAP_WIDTH - 1;
                        spawn.y = y;
                    }
                    (WorldKind::Plaine, WorldKind::Spirale2) => {
                        spawn.x = MAP_WIDTH - 1;
                        spawn.y = y;
                    }
                    (WorldKind::Spirale2, WorldKind::Plaine) => {
                        spawn.x = 0;
                        spawn.y = y;
                    }
                    _ => {}
                }
            }
        }
        self.move_player_to(spawn);
        true
    }

    fn store_current_world_chests(&mut self) {
        self.chests.store_world_snapshot(self.current_world);
    }

    fn apply_player_attack_bonus(&mut self) {
        let mut values = None;
        if let Ok(mut world) = self.world.lock() {
            if let Some(player) = world.players_mut().get_mut(0) {
                let stats = player.stats_mut();
                let before = stats.attaque;
                let after = stats.attaque.saturating_mul(2);
                stats.attaque = after;
                values = Some((before, after));
            }
        }
        if let Some((before, after)) = values {
            self.messages.push(Message {
                texte: format!("Votre attaque passe de {} à {} !", before, after),
                timer: 2.5,
                centered: true,
            });
        }
    }
}

fn choose_grass_variants(textures: &GameTextures, tiles: &[Vec<TileType>]) -> Vec<Vec<usize>> {
    let mut rng = thread_rng();
    (0..MAP_HEIGHT)
        .map(|y| {
            (0..MAP_WIDTH)
                .map(|x| {
                    if tiles[y][x] == TileType::Herbe {
                        rng.gen_range(0..textures.grass_variants.len())
                    } else {
                        0
                    }
                })
                .collect::<Vec<usize>>()
        })
        .collect()
}

fn choose_chemin_variants(textures: &GameTextures, tiles: &[Vec<TileType>]) -> Vec<Vec<usize>> {
    if textures.chemin_variants.is_empty() {
        return vec![vec![0; MAP_WIDTH]; MAP_HEIGHT];
    }
    let mut rng = thread_rng();
    (0..MAP_HEIGHT)
        .map(|y| {
            (0..MAP_WIDTH)
                .map(|x| {
                    if tiles[y][x] == TileType::Chemin {
                        rng.gen_range(0..textures.chemin_variants.len())
                    } else {
                        0
                    }
                })
                .collect::<Vec<usize>>()
        })
        .collect()
}

fn spawn_random_enemies(
    world: &mut World,
    tiles: &[Vec<TileType>],
    max_enemies: usize,
    world_kind: WorldKind,
) {
    let mut rng = thread_rng();
    let mut next_id = 0;
    let is_spirale2 = matches!(world_kind, WorldKind::Spirale2);
    while world.enemies.len() < max_enemies {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        if !matches!(tiles[y][x], TileType::Herbe | TileType::Chemin) {
            continue;
        }
        let tile_free = world.players().iter().filter(|p| p.est_vivant()).all(|p| {
            let pos = p.position();
            pos.x != x || pos.y != y
        }) && world.enemies().iter().filter(|e| e.est_vivant()).all(|e| {
            let pos = e.position();
            pos.x != x || pos.y != y
        });
        if tile_free {
            let mut ennemi = Ennemi::nouveau(next_id, Position { x, y });
            if is_spirale2 {
                let stats = ennemi.stats_mut();
                stats.attaque *= 2;
            }
            world.add_enemy(ennemi);
            next_id += 1;
        }
    }
}

fn start_enemy_thread(world: &Arc<Mutex<World>>) {
    let thread_world = Arc::clone(world);
    thread::spawn(move || {
        let tick_ms = 400u64;
        let dt = (tick_ms as f32) / 1000.0;
        loop {
            if let Ok(mut world) = thread_world.lock() {
                world.wander_enemies(dt);
            }
            thread::sleep(Duration::from_millis(tick_ms));
        }
    });
}

fn parse_world_file(path: &Path) -> Result<Vec<Vec<TileType>>, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Impossible de lire {:?}: {}", path, e))?;
    let mut rows = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut row = Vec::new();
        for token in trimmed.split(|c: char| c.is_whitespace() || c == ',') {
            if token.is_empty() {
                continue;
            }
            row.push(parse_tile_value(token)?);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.len() != MAP_HEIGHT {
        return Err(format!(
            "Le fichier {:?} contient {} lignes au lieu de {}",
            path,
            rows.len(),
            MAP_HEIGHT
        ));
    }
    for (y, row) in rows.iter().enumerate() {
        if row.len() != MAP_WIDTH {
            return Err(format!(
                "La ligne {} dans {:?} contient {} colonnes au lieu de {}",
                y,
                path,
                row.len(),
                MAP_WIDTH
            ));
        }
    }
    Ok(rows)
}

fn parse_tile_value(token: &str) -> Result<TileType, String> {
    let value: i32 = token
        .parse()
        .map_err(|_| format!("Valeur de tuile invalide: {}", token))?;
    match value {
        -4 => Ok(TileType::Coffre),
        -3 => Ok(TileType::CollisionInvisible),
        -2 => Ok(TileType::Maison),
        -1 => Ok(TileType::Eau),
        0 => Ok(TileType::Herbe),
        1 | 2 => Ok(TileType::Chemin),
        3 => Ok(TileType::Portal),
        4 => Ok(TileType::SpiralPortal),
        5 => Ok(TileType::SpiralPortal2),
        other => Err(format!("Code tuile inconnu: {}", other)),
    }
}

fn detect_house_anchors(tiles: &[Vec<TileType>]) -> Vec<Position> {
    let mut anchors = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if tiles[y][x] != TileType::Maison {
                continue;
            }
            let left_is_house = x > 0 && tiles[y][x - 1] == TileType::Maison;
            let top_is_house = y > 0 && tiles[y - 1][x] == TileType::Maison;
            if !left_is_house && !top_is_house {
                anchors.push(Position { x, y });
            }
        }
    }
    anchors
}

fn default_map_tiles() -> Vec<Vec<TileType>> {
    let mut tiles = vec![vec![TileType::Herbe; MAP_WIDTH]; MAP_HEIGHT];
    for y in 0..=PLAYER_START_Y.min(MAP_HEIGHT - 1) {
        tiles[y][PLAYER_START_X] = TileType::Chemin;
    }
    if PLAYER_START_Y < MAP_HEIGHT {
        for x in PLAYER_START_X..MAP_WIDTH {
            tiles[PLAYER_START_Y][x] = TileType::Chemin;
        }
    }
    tiles
}

fn load_tiles_for_world(kind: WorldKind) -> Vec<Vec<TileType>> {
    let target = Path::new("worlds").join(kind.filename());
    parse_world_file(&target).unwrap_or_else(|_| load_first_world_in_dir())
}

fn load_first_world_in_dir() -> Vec<Vec<TileType>> {
    let base = Path::new("worlds");
    let _ = fs::create_dir_all(base);
    let mut files = match fs::read_dir(base) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|entry| entry.path())
            .collect::<Vec<_>>(),
        Err(_) => return default_map_tiles(),
    };
    files.sort();
    for path in files {
        if let Ok(tiles) = parse_world_file(&path) {
            return tiles;
        }
    }
    default_map_tiles()
}

fn build_walkable_map(tiles: &[Vec<TileType>]) -> Vec<Vec<bool>> {
    tiles
        .iter()
        .map(|row| {
            row.iter()
                .map(|tile| matches!(tile, TileType::Herbe | TileType::Chemin | TileType::Portal | TileType::SpiralPortal | TileType::SpiralPortal2) && !matches!(tile, TileType::CollisionInvisible))
                .collect::<Vec<bool>>()
        })
        .collect()
}

fn find_tile_position(tiles: &[Vec<TileType>], needle: TileType) -> Option<Position> {
    for (y, row) in tiles.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            if *tile == needle {
                return Some(Position { x, y });
            }
        }
    }
    None
}

fn enemy_cap(kind: WorldKind) -> usize {
    match kind {
        WorldKind::Plaine => MAX_ENEMIES,
        WorldKind::Maison => 0,
        WorldKind::Spirale => 5,
        WorldKind::Spirale2 => 5,
    }
}
