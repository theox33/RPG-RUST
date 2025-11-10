pub mod combat;

use crate::entity::{Classe, Combatant, Personnage, Position};
use crate::world::World;
use combat::{CombatInput, CombatResolution, CombatState, CombatTransition};
use macroquad::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const TILE_SIZE: f32 = 48.0;

pub struct Game {
    world: Arc<Mutex<World>>,
    state: GameState,
    messages: Vec<Message>,
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
    pub fn new() -> Self {
        let mut world = World::new(12, 8);
        world.add_player(Personnage::nouveau_joueur(0, Classe::Soldat, Position { x: 2, y: 3 }));
        world.add_enemy(Personnage::nouvel_ennemi(0, Classe::Assassin, Position { x: 9, y: 3 }));
        world.add_enemy(Personnage::nouvel_ennemi(1, Classe::Soldat, Position { x: 9, y: 5 }));

        let world = Arc::new(Mutex::new(world));
        let thread_world = Arc::clone(&world);
        thread::spawn(move || {
            // Tick fixe pour le déplacement autonome aléatoire
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
        }
    }

    pub fn frame(&mut self) {
        clear_background(LIGHTGRAY);
        let dt = get_frame_time();
        self.update_messages(dt);
        if matches!(self.state, GameState::Exploration) {
            self.update_exploration();
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

    fn update_exploration(&mut self) {
        let mut dx: isize = 0;
        let mut dy: isize = 0;
        if is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Z) { dy = -1; }
        if is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::S) { dy = 1; }
        if is_key_pressed(KeyCode::Left) || is_key_pressed(KeyCode::Q) { dx = -1; }
        if is_key_pressed(KeyCode::Right) || is_key_pressed(KeyCode::D) { dx = 1; }

        let mut world = self.world.lock().unwrap();
        if dx != 0 || dy != 0 {
            let moved = world.move_player(0, dx, dy);
            if moved {
                self.messages.push(Message { texte: "Vous vous déplacez.".to_string(), timer: 0.6 });
            }
        }

        if let Some((p_idx, e_idx)) = world.find_adjacent_pair() {
            self.messages.push(Message { texte: "Un ennemi est proche : combat engagé !".to_string(), timer: 1.2 });
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
                draw_rectangle(x_f, y_f, TILE_SIZE - 2.0, TILE_SIZE - 2.0, WHITE);
            }
        }
    }

    fn draw_entities(&self, world: &World) {
        for p in world.players() {
            if p.est_vivant() {
                let pos = p.position();
                let x_f = pos.x as f32 * TILE_SIZE;
                let y_f = pos.y as f32 * TILE_SIZE;
                draw_rectangle(x_f + 4.0, y_f + 4.0, TILE_SIZE - 8.0, TILE_SIZE - 8.0, BLUE);
            }
        }
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
