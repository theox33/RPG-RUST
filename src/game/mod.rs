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
/// d'animation (`char_texture`) contient 3 colonnes et 3 lignes de frames ;
/// les rectangles correspondants sont stockés dans `char_frames`.
pub struct GameTextures {
    pub map_texture: Texture2D,
    pub herbe_src: Rect,
    pub chemin_src: Rect,
    pub char_texture: Texture2D,
    pub char_frames: Vec<Rect>,
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
/// la direction : 1 frame pour la position statique (vers le bas), 2 frames
/// pour marcher vers la droite, 3 frames pour marcher vers la gauche et
/// 3 frames pour marcher vers le haut.
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
    /// Si le personnage se déplace, l'index de frame est incrémenté lorsque
    /// le temps accumulé dépasse `frame_duration`. Sinon, l'animation est
    /// réinitialisée.
    fn update(&mut self, dt: f32, moving: bool) {
        if moving {
            self.timer += dt;
            if self.timer > self.frame_duration {
                self.timer -= self.frame_duration;
                // Chaque direction possède désormais 3 frames
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
            // La spritesheet est organisée en 4 lignes de 3 frames chacune :
            // ligne 0 = bas, ligne 1 = droite, ligne 2 = gauche, ligne 3 = haut.
            Direction::Down => 0 + self.frame,
            Direction::Right => 3 + self.frame,
            Direction::Left => 6 + self.frame,
            Direction::Up => 9 + self.frame,
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
    /// Indique si le joueur est en train de glisser d'une case à l'autre.
    moving: bool,
    /// Temps total en secondes pour parcourir une case lors du déplacement.
    move_time: f32,
    /// Progrès du déplacement courant (entre 0.0 et 1.0).
    move_progress: f32,
    /// Coordonnées de départ du déplacement en cours.
    move_from: Position,
    /// Coordonnées d'arrivée du déplacement en cours.
    move_to: Position,
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
        world.add_player(Personnage::nouveau_joueur(
            0,
            Classe::Soldat,
            Position {
                x: PLAYER_START_X,
                y: PLAYER_START_Y,
            },
        ));

        // Générer des ennemis sur les cases d'herbe libres
        spawn_random_enemies(&mut world, &map_tiles, MAX_ENEMIES);

        // Définir les sous-rectangles pour l'herbe et le chemin. Ces valeurs
        // sont calculées manuellement à partir de la feuille de texture.
        let herbe_src = Rect::new(20.0, 20.0, 100.0, 100.0);
        let chemin_src = Rect::new(300.0, 20.0, 100.0, 100.0);

        // Créer la liste de frames pour l'animation du personnage (3 colonnes × 4 lignes).
        // Les frames sont décalées de quelques pixels vers le bas (offset_y) pour éviter
        // d'inclure des pixels de la ligne supérieure.
        // La spritesheet du personnage est organisée en 3 colonnes × 3 lignes.
        // Nous appliquons un léger décalage vertical (offset_y) pour éviter
        // d’afficher des pixels résiduels provenant de la ligne supérieure.
        // De plus, la hauteur de découpe est réduite de ce même offset pour
        // n’extraire que la zone utile de chaque frame.
        let cols = 3;
        // La spritesheet comporte maintenant 4 rangées (bas, droite, gauche, haut)
        let rows = 4;
        let cw = char_texture.width() / cols as f32;
        let ch = char_texture.height() / rows as f32;
        // Décalage vertical en pixels pour ignorer la bordure entre les frames.
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
            moving: false,
            // Durée d'une glissade entre deux cases (en secondes). Augmentée pour que l'animation soit plus visible.
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
        }
    }

    pub fn frame(&mut self) {
        clear_background(LIGHTGRAY);
        let dt = get_frame_time();
        self.update_messages(dt);
        // D'abord mettre à jour l'avancement du déplacement en cours avant toute logique de jeu.
        if matches!(self.state, GameState::Exploration) {
            self.update_movement(dt);
            self.update_exploration(dt);
        } else {
            self.update_combat_state();
        }
        self.render();
    }

    /// Met à jour l'animation de déplacement. Lorsque `moving` est vrai, on
    /// incrémente `move_progress` en fonction du temps écoulé et on termine le
    /// mouvement lorsque la valeur dépasse 1.0.
    fn update_movement(&mut self, dt: f32) {
        if self.moving {
            self.move_progress += dt / self.move_time;
            if self.move_progress >= 1.0 {
                self.moving = false;
                self.move_progress = 0.0;
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
        // Si le joueur est en train de glisser d'une case à l'autre, on met simplement
        // à jour l'animation et on retourne sans tenter de lancer un nouveau déplacement.
        if self.moving {
            // Mettre à jour l'animation pendant la glisse.
            self.player_anim.update(dt, true);
            return;
        }

        // Déplacement sur la grille : on déplace le joueur d'une case par pression de touche,
        // mais l'animation doit continuer tant que la touche reste enfoncée. On sépare donc
        // la détection du mouvement (pour déplacer) et la détection de l'appui continu (pour animer).
        let mut dx: isize = 0;
        let mut dy: isize = 0;

        // Déterminer le déplacement souhaité en fonction des touches enfoncées (QWERTY et AZERTY).
        // On utilise `is_key_down` afin de permettre la répétition du mouvement quand la touche est maintenue.
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

        // Le personnage est en mouvement si au moins une touche directionnelle est enfoncée.
        let moving_input = dx != 0 || dy != 0;

        if moving_input {
            // Choisir la direction en fonction des touches maintenues. L'ordre de
            // priorité est donné par la superposition verticale puis horizontale :
            // les déplacements horizontaux écrasent les déplacements verticaux.
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
        // Mettre à jour l'animation du joueur en fonction du temps écoulé et de l'appui
        self.player_anim.update(dt, moving_input);

        let mut world = self.world.lock().unwrap();
        // Déplacer le joueur d'une case lorsque l'on détecte une pression de touche
        if dx != 0 || dy != 0 {
            // Mémoriser la position avant déplacement
            let old_pos = world.players()[0].position();
            let moved = world.move_player(0, dx, dy);
            if moved {
                // Mémoriser la position après déplacement
                let new_pos = world.players()[0].position();
                // Initialiser l'interpolation de déplacement
                self.moving = true;
                self.move_progress = 0.0;
                self.move_from = Position {
                    x: old_pos.x,
                    y: old_pos.y,
                };
                self.move_to = Position {
                    x: new_pos.x,
                    y: new_pos.y,
                };
                // Réinitialiser l'animation pour démarrer sur la première frame
                self.player_anim.frame = 0;
                self.player_anim.timer = 0.0;
                // Adapter la durée d'une frame en fonction du nombre de frames pour cette direction (3 frames par direction)
                let nframes = 3;
                self.player_anim.frame_duration = self.move_time / nframes as f32;
                self.messages.push(Message {
                    texte: String::from("Vous vous déplacez."),
                    timer: 0.6,
                });
                // Vérifier immédiatement si un combat doit être lancé
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
                // Déterminer la position affichée. Si un déplacement est en cours, on interpole
                // entre les coordonnées d'origine et de destination selon move_progress.
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
        // Dessiner les ennemis comme des rectangles rouges pour l'instant
        for e in world.enemies() {
            if e.est_vivant() {
                let pos = e.position();
                let x_f = pos.x as f32 * TILE_SIZE;
                let y_f = pos.y as f32 * TILE_SIZE;
                draw_rectangle(
                    x_f + 6.0,
                    y_f + 6.0,
                    TILE_SIZE - 12.0,
                    TILE_SIZE - 12.0,
                    RED,
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
/// `(start_x, start_y)` et s'étend vers le haut jusqu'en Y=0 puis vers la droite
/// jusqu'à `width - 1`.
fn generate_map_tiles(
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
) -> Vec<Vec<TileType>> {
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