pub mod combat;

use crate::ennemi::Ennemi;
use crate::joueur::Joueur;
use crate::types::{Combatant, Position};
use crate::world::World;
use ::rand::{thread_rng, Rng};
use combat::{CombatInput, CombatResolution, CombatResult, CombatState, CombatTransition};
use macroquad::prelude::*;
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TileType {
    Herbe,
    Chemin,
    Eau,
}

pub struct GameTextures {
    pub map_texture: Texture2D,
    pub chemin_src: Rect,
    pub chemin_variants: Vec<Rect>,
    pub char_texture: Texture2D,
    pub char_frames: Vec<Rect>,
    pub grass_variants: Vec<Rect>,
    pub enemy_texture: Texture2D,
    pub enemy_frames: Vec<Rect>,
    pub water_src: Rect,
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
}

enum GameState {
    Exploration,
    Combat(CombatState),
}

pub struct Game {
    world: Arc<Mutex<World>>,
    state: GameState,
    messages: Vec<Message>,
    textures: GameTextures,
    map_tiles: Vec<Vec<TileType>>,
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
}

impl Game {
    pub fn new(map_texture: Texture2D, char_texture: Texture2D, enemy_texture: Texture2D) -> Self {
        let chemin_src = Rect::new(300.0, 20.0, 100.0, 100.0);
        let chemin_variants = vec![
            Rect::new(450.0, 20.0, 100.0, 100.0),
            Rect::new(595.0, 20.0, 100.0, 100.0),
            Rect::new(740.0, 20.0, 100.0, 100.0),
        ];
        let water_src = Rect::new(170.0, 170.0, 100.0, 100.0);
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
        let textures = GameTextures {
            map_texture,
            chemin_src,
            chemin_variants,
            char_texture,
            char_frames,
            grass_variants,
            enemy_texture,
            enemy_frames,
            water_src,
        };

        let mut map_tiles = load_world_tiles();
        if map_tiles.len() != MAP_HEIGHT || map_tiles[0].len() != MAP_WIDTH {
            map_tiles = default_map_tiles();
        }

        let mut world = World::new(MAP_WIDTH, MAP_HEIGHT);
        world.add_player(Joueur::nouveau(
            0,
            Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
        ));
        spawn_random_enemies(&mut world, &map_tiles, MAX_ENEMIES);
        let enemy_prev_positions = world
            .enemies()
            .iter()
            .map(|e| e.position())
            .collect::<Vec<_>>();
        let enemy_anim = enemy_prev_positions
            .iter()
            .map(|pos| EnemyAnim {
                moving: false,
                from: *pos,
                to: *pos,
                progress: 0.0,
                frame: 0,
                timer: 0.0,
                frame_duration: 0.0,
            })
            .collect();
        let grass_choice = choose_grass_variants(&textures, &map_tiles);
        let chemin_choice = choose_chemin_variants(&textures, &map_tiles);
        let world = Arc::new(Mutex::new(world));
        start_enemy_thread(&world);
        Self {
            world,
            state: GameState::Exploration,
            messages: Vec::new(),
            textures,
            map_tiles,
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
            grass_choice,
            chemin_choice,
            enemy_anim,
            enemy_prev_positions,
        }
    }

    pub fn frame(&mut self) {
        clear_background(LIGHTGRAY);
        let dt = get_frame_time();
        self.update_messages(dt);
        if matches!(self.state, GameState::Exploration) {
            self.update_movement(dt);
            self.update_enemy_movement(dt);
            self.update_exploration(dt);
        } else {
            self.update_combat_state();
        }
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
                if nx < MAP_WIDTH
                    && ny < MAP_HEIGHT
                    && matches!(self.map_tiles[ny][nx], TileType::Eau)
                {
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
            });
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
        let input = CombatInput {
            keys_pressed: keys,
            mouse_click,
            tile_size: TILE_SIZE,
            world_height: world.height,
        };
        let CombatResolution {
            messages,
            transition,
        } = state.update(&mut world, &input);
        for msg in messages {
            self.messages.push(Message {
                texte: msg.texte,
                timer: msg.duree,
            });
        }
        match transition {
            CombatTransition::Continue => true,
            CombatTransition::Terminer(result) => {
                if let CombatResult::EnnemiVainqueur | CombatResult::DoubleKo = result {
                    self.messages.push(Message {
                        texte: String::from("Vous êtes vaincu..."),
                        timer: 2.0,
                    });
                }
                false
            }
        }
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
        if let Ok(world) = self.world.lock() {
            self.draw_player(&world, origin_x, origin_y);
            self.draw_enemies(&world, origin_x, origin_y);
            if let GameState::Combat(state) = &self.state {
                state.draw_ui(&world, TILE_SIZE);
            }
        }
        self.draw_messages();
    }

    fn world_origin(&self) -> (f32, f32) {
        let world_w = MAP_WIDTH as f32 * TILE_SIZE;
        let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
        let origin_x = ((screen_width() - world_w) * 0.5).max(0.0);
        let origin_y = ((screen_height() - world_h) * 0.5).max(0.0);
        (origin_x, origin_y)
    }

    fn draw_tiles(&self, origin_x: f32, origin_y: f32) {
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let tile = self.map_tiles[y][x];
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
        for msg in &self.messages {
            draw_text(&msg.texte, 20.0, y, 22.0, DARKGRAY);
            y += 24.0;
        }
    }

    fn tile_walkable(&self, x: usize, y: usize) -> bool {
        matches!(self.map_tiles[y][x], TileType::Herbe | TileType::Chemin)
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

fn spawn_random_enemies(world: &mut World, tiles: &[Vec<TileType>], max_enemies: usize) {
    let mut rng = thread_rng();
    let mut next_id = 0;
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
            world.add_enemy(Ennemi::nouveau(next_id, Position { x, y }));
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

fn load_world_tiles() -> Vec<Vec<TileType>> {
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
        -1 => Ok(TileType::Eau),
        0 => Ok(TileType::Herbe),
        1 | 2 => Ok(TileType::Chemin),
        other => Err(format!("Code tuile inconnu: {}", other)),
    }
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
