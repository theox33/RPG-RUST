pub mod combat;

use crate::entity::{Classe, Combatant, Personnage, Position};
use crate::world::World;
use combat::{CombatInput, CombatResolution, CombatState, CombatTransition};
use macroquad::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use ::rand::{thread_rng, Rng};

/// Taille (en pixels) d'une cellule de la grille. Ajustez pour zoomer/dézoomer.
pub const TILE_SIZE: f32 = 48.0;

/// Largeur de la carte (nombre de colonnes).
pub const MAP_WIDTH: usize = 20;
/// Hauteur de la carte (nombre de lignes).
pub const MAP_HEIGHT: usize = 12;
/// Coordonnée X initiale du joueur.
pub const PLAYER_START_X: usize = 2;
/// Coordonnée Y initiale du joueur.
pub const PLAYER_START_Y: usize = 3;
/// Nombre maximum d'ennemis générés par carte.
pub const MAX_ENEMIES: usize = 4;

/// Types de tuiles présents dans la carte. `Herbe` est un sol classique où
/// les ennemis peuvent apparaître; `Chemin` représente un chemin sablonneux
/// reliant le point de départ aux bords de la carte.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum TileType {
    Herbe,
    Chemin,
}

/// Regroupe toutes les textures nécessaires au rendu et leurs rectangles de
/// découpe. La feuille de carte (`map_texture`) est découpée en deux zones :
/// l'herbe (`herbe_src`) et le chemin (`chemin_src`). La feuille
/// d'animation (`char_texture`) contient 3 colonnes et 4 lignes de frames ;
/// les rectangles correspondants sont stockés dans `char_frames`. Les ennemis
/// utilisent une feuille séparée (`enemy_texture`) découpée en 2 colonnes × 3
/// lignes pour 6 frames.
pub struct GameTextures {
    pub map_texture: Texture2D,
    pub herbe_src: Rect,
    pub chemin_src: Rect,
    pub char_texture: Texture2D,
    pub char_frames: Vec<Rect>,
    pub grass_variants: Vec<Rect>,
    pub enemy_texture: Texture2D,
    pub enemy_frames: Vec<Rect>,
}

/// Directions possibles du personnage. Chaque direction dispose d'un nombre
/// différent de frames d'animation.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Direction {
    Down,
    Right,
    Left,
    Up,
}

/// Animation du personnage principal. Stocke la direction courante du personnage,
/// l'indice de frame en cours et un accumulateur de temps pour faire défiler
/// les animations. Le personnage possède un nombre variable de frames selon
/// la direction : 3 frames pour chaque direction.
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

    /// Met à jour l'animation en fonction du temps écoulé et du déplacement.
    fn update(&mut self, dt: f32, moving: bool) {
        if moving {
            self.timer += dt;
            if self.timer > self.frame_duration {
                self.timer -= self.frame_duration;
                let max_frames = 3;
                self.frame = (self.frame + 1) % max_frames;
            }
        } else {
            self.frame = 0;
            self.timer = 0.0;
        }
    }

    /// Retourne l'indice du rectangle source à utiliser dans la spritesheet.
    fn current_frame_index(&self) -> usize {
        match self.direction {
            Direction::Down => 0 + self.frame,
            Direction::Right => 3 + self.frame,
            Direction::Left => 6 + self.frame,
            Direction::Up => 9 + self.frame,
        }
    }
}

/// Animation et état de déplacement d'un ennemi (slime). Chaque déplacement
/// utilise 6 frames : 3 pour la montée (colonne gauche) et 3 pour
/// l'atterrissage (colonne droite lue de bas en haut).
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
    enemy_anim: Vec<EnemyAnim>,
    enemy_prev_positions: Vec<Position>,
}

enum GameState {
    Exploration,
    Combat(CombatState),
}

struct Message {
    texte: String,
    timer: f32,
}

impl Game {
    /// Crée un nouveau jeu à partir des textures fournies. La carte est
    /// générée avec de l'herbe et un chemin. Les ennemis utilisent une feuille
    /// de texture séparée pour leur animation.
    pub fn new(map_texture: Texture2D, char_texture: Texture2D, enemy_texture: Texture2D) -> Self {
        // Générer la grille de la carte
        let map_tiles = generate_map_tiles(MAP_WIDTH, MAP_HEIGHT, PLAYER_START_X, PLAYER_START_Y);
        // Initialiser le monde et ajouter le joueur
        let mut world = World::new(MAP_WIDTH, MAP_HEIGHT);
        world.add_player(Personnage::nouveau_joueur(
            0,
            Classe::Soldat,
            Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
        ));
        // Générer des ennemis sur les cases d'herbe
        spawn_random_enemies(&mut world, &map_tiles, MAX_ENEMIES);
        // Définir les rectangles sources pour l'herbe et le chemin
        let herbe_src = Rect::new(20.0, 20.0, 100.0, 100.0);
        let chemin_src = Rect::new(300.0, 20.0, 100.0, 100.0);
        // Variantes d'herbe (5 variantes)
        let grass_variants = vec![
            Rect::new(20.0, 20.0, 100.0, 100.0),
            Rect::new(170.0, 20.0, 100.0, 100.0),
            Rect::new(20.0, 170.0, 100.0, 100.0),
            Rect::new(450.0, 170.0, 100.0, 100.0),
            Rect::new(450.0, 320.0, 100.0, 100.0),
        ];
        // Découper les frames du personnage (3 colonnes × 4 lignes)
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
        // Découper les frames de l'ennemi (2 colonnes × 3 lignes)
        let e_cols = 2;
        let e_rows = 3;
        let ecw = enemy_texture.width() / e_cols as f32;
        let ech = enemy_texture.height() / e_rows as f32;
        let mut enemy_frames = Vec::new();
        // Colonne gauche : montée
        for row in 0..e_rows {
            enemy_frames.push(Rect::new(0.0, row as f32 * ech, ecw, ech));
        }
        // Colonne droite : atterrissage (du bas vers le haut)
        for row in (0..e_rows).rev() {
            enemy_frames.push(Rect::new(ecw, row as f32 * ech, ecw, ech));
        }
        // Construire la structure GameTextures
        let textures = GameTextures {
            map_texture,
            herbe_src,
            chemin_src,
            char_texture,
            char_frames,
            grass_variants,
            enemy_texture,
            enemy_frames,
        };
        // Capturer les positions initiales des ennemis avant de placer le monde dans un Arc
        let initial_enemy_positions: Vec<Position> = world
            .enemies()
            .iter()
            .map(|e| e.position())
            .collect();
        let enemy_anim = initial_enemy_positions
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
            .collect::<Vec<EnemyAnim>>();
        let enemy_prev_positions = initial_enemy_positions.clone();
        // Partager le monde entre threads
        let world = Arc::new(Mutex::new(world));
        let thread_world = Arc::clone(&world);
        thread::spawn(move || {
            let tick_ms = 400u64;
            let dt = (tick_ms as f32) / 1000.0;
            loop {
                {
                    let mut world = thread_world.lock().unwrap();
                    world.wander_enemies(dt);
                }
                thread::sleep(Duration::from_millis(tick_ms));
            }
        });
        // Choisir une variante d'herbe pour chaque case d'herbe
        let mut rng = thread_rng();
        let grass_choice = (0..MAP_HEIGHT)
            .map(|y| {
                (0..MAP_WIDTH)
                    .map(|x| {
                        if map_tiles[y][x] == TileType::Herbe {
                            rng.gen_range(0..textures.grass_variants.len())
                        } else {
                            0usize
                        }
                    })
                    .collect::<Vec<usize>>()
            })
            .collect::<Vec<Vec<usize>>>();
        // Retourner la structure Game
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
            move_from: Position { x: PLAYER_START_X, y: PLAYER_START_Y },
            move_to: Position { x: PLAYER_START_X, y: PLAYER_START_Y },
            grass_choice,
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

    /// Met à jour la progression du déplacement du joueur.
    fn update_movement(&mut self, dt: f32) {
        if self.moving {
            self.move_progress += dt / self.move_time;
            if self.move_progress >= 1.0 {
                self.moving = false;
                self.move_progress = 0.0;
            }
        }
    }

    /// Met à jour l'animation et la glissade des ennemis.
    fn update_enemy_movement(&mut self, dt: f32) {
        let current_positions: Vec<Position> = {
            let world = self.world.lock().unwrap();
            world.enemies().iter().map(|e| e.position()).collect()
        };
        while self.enemy_anim.len() < current_positions.len() {
            self.enemy_anim.push(EnemyAnim {
                moving: false,
                from: current_positions[self.enemy_anim.len()],
                to: current_positions[self.enemy_anim.len()],
                progress: 0.0,
                frame: 0,
                timer: 0.0,
                frame_duration: 0.0,
            });
            self.enemy_prev_positions.push(current_positions[self.enemy_prev_positions.len()]);
        }
        for i in 0..current_positions.len() {
            let pos = current_positions[i];
            let prev = self.enemy_prev_positions[i];
            let anim = &mut self.enemy_anim[i];
            if anim.moving {
                anim.progress += dt / self.move_time;
                anim.timer += dt;
                if anim.frame_duration > 0.0 && anim.timer > anim.frame_duration {
                    anim.timer -= anim.frame_duration;
                    anim.frame = (anim.frame + 1) % 6;
                }
                if anim.progress >= 1.0 {
                    anim.moving = false;
                    anim.progress = 0.0;
                    anim.frame = 0;
                    anim.timer = 0.0;
                    self.enemy_prev_positions[i] = anim.to;
                }
            } else {
                if pos.x != prev.x || pos.y != prev.y {
                    anim.moving = true;
                    anim.from = prev;
                    anim.to = pos;
                    anim.progress = 0.0;
                    anim.frame = 0;
                    anim.timer = 0.0;
                    anim.frame_duration = self.move_time / 6.0;
                    self.enemy_prev_positions[i] = pos;
                }
            }
        }
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
        let mut world = self.world.lock().unwrap();
        if dx != 0 || dy != 0 {
            let old_pos = world.players()[0].position();
            let moved = world.move_player(0, dx, dy);
            if moved {
                let new_pos = world.players()[0].position();
                self.moving = true;
                self.move_progress = 0.0;
                self.move_from = Position { x: old_pos.x, y: old_pos.y };
                self.move_to = Position { x: new_pos.x, y: new_pos.y };
                self.player_anim.frame = 0;
                self.player_anim.timer = 0.0;
                let nframes = 3;
                self.player_anim.frame_duration = self.move_time / nframes as f32;
                self.messages.push(Message {
                    texte: String::from("Vous vous déplacez."),
                    timer: 0.6,
                });
                if let Some((p_idx, e_idx)) = world.find_adjacent_pair() {
                    self.messages.push(Message {
                        texte: String::from("Un ennemi est proche : combat engagé !"),
                        timer: 1.2,
                    });
                    let player_speed = world.players()[p_idx].vitesse();
                    let enemy_speed = world.enemies()[e_idx].vitesse();
                    let player_first = player_speed >= enemy_speed;
                    let combat_state = CombatState::with_initiative(p_idx, e_idx, player_first);
                    self.state = GameState::Combat(combat_state);
                }
            }
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
        let mut world = self.world.lock().unwrap();
        let input = CombatInput {
            keys_pressed: keys,
            mouse_click,
            tile_size: TILE_SIZE,
            world_height: world.height,
        };
        let CombatResolution { messages, transition } = state.update(&mut world, &input);
        drop(world);
        for msg in messages {
            self.messages.push(Message {
                texte: msg.texte,
                timer: msg.duree,
            });
        }
        !matches!(transition, CombatTransition::Terminer(_))
    }

    fn update_combat_state(&mut self) {
        let current = std::mem::replace(&mut self.state, GameState::Exploration);
        if let GameState::Combat(mut state) = current {
            let still_fighting = self.update_combat(&mut state);
            if still_fighting {
                self.state = GameState::Combat(state);
            } else {
                self.state = GameState::Exploration;
            }
        } else {
            self.state = current;
        }
    }

    fn render(&self) {
        let world = self.world.lock().unwrap();
        self.draw_grid(&world);
        self.draw_entities(&world);
        if let GameState::Combat(state) = &self.state {
            state.draw_ui(&world, TILE_SIZE);
        }
        drop(world);
        self.draw_messages();
    }

    fn draw_grid(&self, world: &World) {
        for y in 0..world.height {
            for x in 0..world.width {
                let x_f = x as f32 * TILE_SIZE;
                let y_f = y as f32 * TILE_SIZE;
                match self.map_tiles[y][x] {
                    TileType::Herbe => {
                        let variant_index = self.grass_choice[y][x];
                        let src = self.textures.grass_variants[variant_index];
                        draw_texture_ex(
                            &self.textures.map_texture,
                            x_f,
                            y_f,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                                source: Some(src),
                                ..Default::default()
                            },
                        );
                    }
                    TileType::Chemin => {
                        draw_texture_ex(
                            &self.textures.map_texture,
                            x_f,
                            y_f,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                                source: Some(self.textures.chemin_src),
                                ..Default::default()
                            },
                        );
                    }
                }
            }
        }
    }

    fn draw_entities(&self, world: &World) {
        for p in world.players() {
            if p.est_vivant() {
                let (x_f, y_f) = if self.moving {
                    let from_x = self.move_from.x as f32;
                    let from_y = self.move_from.y as f32;
                    let to_x = self.move_to.x as f32;
                    let to_y = self.move_to.y as f32;
                    let interp_x = from_x + (to_x - from_x) * self.move_progress;
                    let interp_y = from_y + (to_y - from_y) * self.move_progress;
                    (interp_x * TILE_SIZE, interp_y * TILE_SIZE)
                } else {
                    let pos = p.position();
                    (pos.x as f32 * TILE_SIZE, pos.y as f32 * TILE_SIZE)
                };
                let index = self.player_anim.current_frame_index();
                let src = self.textures.char_frames[index];
                draw_texture_ex(
                    &self.textures.char_texture,
                    x_f,
                    y_f,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                        source: Some(src),
                        flip_x: false,
                        ..Default::default()
                    },
                );
            }
        }
        for (i, e) in world.enemies().iter().enumerate() {
            if e.est_vivant() {
                let anim = &self.enemy_anim[i];
                let (x_f, y_f) = if anim.moving {
                    let from_x = anim.from.x as f32;
                    let from_y = anim.from.y as f32;
                    let to_x = anim.to.x as f32;
                    let to_y = anim.to.y as f32;
                    let interp_x = from_x + (to_x - from_x) * anim.progress;
                    let interp_y = from_y + (to_y - from_y) * anim.progress;
                    (interp_x * TILE_SIZE, interp_y * TILE_SIZE)
                } else {
                    let pos = e.position();
                    (pos.x as f32 * TILE_SIZE, pos.y as f32 * TILE_SIZE)
                };
                let frame_index = self.enemy_anim[i].frame;
                let src = self.textures.enemy_frames[frame_index];
                draw_texture_ex(
                    &self.textures.enemy_texture,
                    x_f,
                    y_f,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                        source: Some(src),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn draw_messages(&self) {
        let mut y = 24.0;
        for msg in &self.messages {
            draw_text(&msg.texte, 10.0, y, 20.0, DARKGRAY);
            y += 22.0;
        }
    }

    fn collect_combat_keys(&self) -> Vec<KeyCode> {
        let mut keys = Vec::new();
        if is_key_pressed(KeyCode::A) {
            keys.push(KeyCode::A);
        }
        if is_key_pressed(KeyCode::D) {
            keys.push(KeyCode::D);
        }
        if is_key_pressed(KeyCode::F) {
            keys.push(KeyCode::F);
        }
        keys
    }
}

/// Génère la grille des tuiles (herbe ou chemin). Le chemin part de
/// `(start_x, start_y)` et s'étend vers le haut puis vers la droite.
fn generate_map_tiles(
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
) -> Vec<Vec<TileType>> {
    let mut tiles = vec![vec![TileType::Herbe; width]; height];
    for y in 0..=start_y {
        tiles[y][start_x] = TileType::Chemin;
    }
    for x in start_x..width {
        tiles[start_y][x] = TileType::Chemin;
    }
    tiles
}

/// Génère des ennemis jusqu'à `max_enemies` sur des cases d'herbe libres.
fn spawn_random_enemies(world: &mut World, tiles: &Vec<Vec<TileType>>, max_enemies: usize) {
    let mut rng = thread_rng();
    let mut next_id = 0;
    while world.enemies.len() < max_enemies {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        if tiles[y][x] == TileType::Herbe {
            let tile_free = world
                .players()
                .iter()
                .filter(|p| p.est_vivant())
                .all(|p| {
                    let pos = p.position();
                    pos.x != x || pos.y != y
                })
                && world
                    .enemies()
                    .iter()
                    .filter(|e| e.est_vivant())
                    .all(|e| {
                        let pos = e.position();
                        pos.x != x || pos.y != y
                    });
            if tile_free {
                world.add_enemy(Personnage::nouvel_ennemi(
                    next_id,
                    Classe::Assassin,
                    Position { x, y },
                ));
                next_id += 1;
            }
        }
    }
}
