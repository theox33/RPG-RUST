pub mod combat;

use crate::entity::{Classe, Combatant, Personnage, Position};
use crate::world::World;
use combat::{CombatInput, CombatResolution, CombatState, CombatTransition};
use macroquad::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use ::rand::{thread_rng, Rng};

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
/// d'animation (`char_texture`) contient 4 colonnes et 3 lignes de frames ;
/// les rectangles correspondants sont stockés dans `char_frames`.
pub struct GameTextures {
    pub map_texture: Texture2D,
    pub herbe_src: Rect,
    pub chemin_src: Rect,
    pub char_texture: Texture2D,
    pub char_frames: Vec<Rect>,
}

/// Animation du personnage principal. Contient la ligne d'animation (`row`),
/// l'indice de frame, un accumulateur de temps pour faire défiler les
/// animations et un indicateur de retournement horizontal (`flip_x`).
#[derive(Clone)]
struct PlayerAnim {
    row: usize,
    frame: usize,
    timer: f32,
    frame_duration: f32,
    flip_x: bool,
}

impl PlayerAnim {
    fn new() -> Self {
        Self {
            row: 0,
            frame: 0,
            timer: 0.0,
            frame_duration: 0.2,
            flip_x: false,
        }
    }
    fn update(&mut self, dt: f32, moving: bool) {
        if moving {
            self.timer += dt;
            if self.timer > self.frame_duration {
                self.timer -= self.frame_duration;
                self.frame = (self.frame + 1) % 4;
            }
        } else {
            self.frame = 0;
            self.timer = 0.0;
        }
    }
}

pub struct Game {
    world: Arc<Mutex<World>>,
    state: GameState,
    messages: Vec<Message>,
    textures: GameTextures,
    map_tiles: Vec<Vec<TileType>>,
    player_anim: PlayerAnim,
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
    /// générée avec une grille d'herbe et un chemin. Le joueur est placé au
    /// point de départ défini par `PLAYER_START_X` et `PLAYER_START_Y` et des
    /// ennemis sont générés aléatoirement sur les cases d'herbe jusqu'à
    /// `MAX_ENEMIES`. Un thread est lancé pour déplacer les ennemis.
    pub fn new(map_texture: Texture2D, char_texture: Texture2D) -> Self {
        // Générer la carte (herbe + chemin)
        let map_tiles = generate_map_tiles(MAP_WIDTH, MAP_HEIGHT, PLAYER_START_X, PLAYER_START_Y);

        // Initialiser le monde et ajouter le joueur
        let mut world = World::new(MAP_WIDTH, MAP_HEIGHT);
        world.add_player(Personnage::nouveau_joueur(0, Classe::Soldat, Position { x: PLAYER_START_X, y: PLAYER_START_Y }));

        // Générer des ennemis sur les cases d'herbe libres
        spawn_random_enemies(&mut world, &map_tiles, MAX_ENEMIES);

        // Définir les sous-rectangles pour l'herbe et le chemin. Ces valeurs
        // sont calculées manuellement à partir de la feuille de texture.
        let herbe_src = Rect::new(20.0, 20.0, 100.0, 100.0);
        let chemin_src = Rect::new(300.0, 20.0, 100.0, 100.0);

        // Créer la liste de frames pour l'animation du personnage (4 colonnes × 3 lignes)
        let cw = char_texture.width() / 4.0;
        let ch = char_texture.height() / 3.0;
        let mut char_frames = Vec::new();
        for row in 0..3 {
            for col in 0..4 {
                char_frames.push(Rect::new(col as f32 * cw, row as f32 * ch, cw, ch));
            }
        }

        let textures = GameTextures {
            map_texture,
            herbe_src,
            chemin_src,
            char_texture,
            char_frames,
        };

        // Partager le monde entre le thread principal et le thread secondaire
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

        Self {
            world,
            state: GameState::Exploration,
            messages: Vec::new(),
            textures,
            map_tiles,
            player_anim: PlayerAnim::new(),
        }
    }

    pub fn frame(&mut self) {
        clear_background(LIGHTGRAY);
        let dt = get_frame_time();
        self.update_messages(dt);
        if matches!(self.state, GameState::Exploration) {
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

    fn update_exploration(&mut self, dt: f32) {
        let mut dx: isize = 0;
        let mut dy: isize = 0;
        // Détecter les touches directionnelles (supporte AZERTY et QWERTY)
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Z) { dy = -1; }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) { dy = 1; }
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Q) { dx = -1; }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) { dx = 1; }

        let moving = dx != 0 || dy != 0;
        if moving {
            // Choisir la ligne d'animation et la symétrie horizontale selon la direction
            if dy < 0 {
                // déplacement vers le haut -> dos (ligne 2)
                self.player_anim.row = 2;
                self.player_anim.flip_x = false;
            } else if dy > 0 {
                // déplacement vers le bas -> face (ligne 0)
                self.player_anim.row = 0;
                self.player_anim.flip_x = false;
            }
            if dx > 0 {
                // mouvement vers la droite -> profil (ligne 1) sans retournement
                self.player_anim.row = 1;
                self.player_anim.flip_x = false;
            } else if dx < 0 {
                // mouvement vers la gauche -> profil (ligne 1) retourné horizontalement
                self.player_anim.row = 1;
                self.player_anim.flip_x = true;
            }
        }
        // Mettre à jour l'animation du joueur
        self.player_anim.update(dt, moving);

        let mut world = self.world.lock().unwrap();
        if moving {
            let moved = world.move_player(0, dx, dy);
            if moved {
                self.messages.push(Message { texte: String::from("Vous vous déplacez."), timer: 0.6 });
            }
        }
        // Lancer un combat si un joueur et un ennemi sont adjacents
        if let Some((p_idx, e_idx)) = world.find_adjacent_pair() {
            self.messages.push(Message { texte: String::from("Un ennemi est proche : combat engagé !"), timer: 1.2 });
            let player_speed = world.players()[p_idx].vitesse();
            let enemy_speed = world.enemies()[e_idx].vitesse();
            let player_first = player_speed >= enemy_speed;
            let combat_state = CombatState::with_initiative(p_idx, e_idx, player_first);
            self.state = GameState::Combat(combat_state);
        }
    }

    fn update_combat(&mut self, state: &mut CombatState) -> bool {
        let keys = self.collect_combat_keys();
        let mouse_click = if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            Some(Vec2::new(mx, my))
        } else { None };
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
            self.messages.push(Message { texte: msg.texte, timer: msg.duree });
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
                        draw_texture_ex(
                            &self.textures.map_texture,
                            x_f,
                            y_f,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                                source: Some(self.textures.herbe_src),
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
        // Dessiner le joueur
        for p in world.players() {
            if p.est_vivant() {
                let pos = p.position();
                let x_f = pos.x as f32 * TILE_SIZE;
                let y_f = pos.y as f32 * TILE_SIZE;
                let index = self.player_anim.row * 4 + self.player_anim.frame;
                let src = self.textures.char_frames[index];
                draw_texture_ex(
                    &self.textures.char_texture,
                    x_f,
                    y_f,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(Vec2::new(TILE_SIZE, TILE_SIZE)),
                        source: Some(src),
                        flip_x: self.player_anim.flip_x,
                        ..Default::default()
                    },
                );
            }
        }
        // Dessiner les ennemis comme des rectangles rouges pour l'instant
        for e in world.enemies() {
            if e.est_vivant() {
                let pos = e.position();
                let x_f = pos.x as f32 * TILE_SIZE;
                let y_f = pos.y as f32 * TILE_SIZE;
                draw_rectangle(x_f + 6.0, y_f + 6.0, TILE_SIZE - 12.0, TILE_SIZE - 12.0, RED);
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
        if is_key_pressed(KeyCode::A) { keys.push(KeyCode::A); }
        if is_key_pressed(KeyCode::D) { keys.push(KeyCode::D); }
        if is_key_pressed(KeyCode::F) { keys.push(KeyCode::F); }
        keys
    }
}

/// Génère la grille des tuiles (herbe ou chemin). Le chemin part de
/// `(start_x, start_y)` et s'étend vers le haut jusqu'en Y=0 puis vers la droite
/// jusqu'à `width - 1`.
fn generate_map_tiles(width: usize, height: usize, start_x: usize, start_y: usize) -> Vec<Vec<TileType>> {
    let mut tiles = vec![vec![TileType::Herbe; width]; height];
    // Chemin vertical vers le haut (y de 0 à start_y inclus)
    for y in 0..=start_y {
        tiles[y][start_x] = TileType::Chemin;
    }
    // Chemin horizontal vers la droite (x de start_x à width-1 inclus)
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
            let tile_free = world.players().iter().filter(|p| p.est_vivant()).all(|p| {
                let pos = p.position();
                pos.x != x || pos.y != y
            }) && world.enemies().iter().filter(|e| e.est_vivant()).all(|e| {
                let pos = e.position();
                pos.x != x || pos.y != y
            });
            if tile_free {
                world.add_enemy(Personnage::nouvel_ennemi(next_id, Classe::Assassin, Position { x, y }));
                next_id += 1;
            }
        }
    }
}
