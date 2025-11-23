use macroquad::prelude::*;

use super::*;

impl Game {
    /// Anime linéairement le déplacement du joueur en fonction du delta temps.
    pub(super) fn update_movement(&mut self, dt: f32) {
        if self.moving {
            self.move_progress += dt / self.move_time;
            if self.move_progress >= 1.0 {
                self.moving = false;
                self.move_progress = 0.0;
            }
        }
    }

    /// Met à jour les animations des ennemis et déclenche un combat en cas de collision.
    pub(super) fn update_enemy_movement(&mut self, dt: f32) {
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

    /// Traite les entrées clavier pour déplacer le joueur sur la carte d'exploration.
    pub(super) fn update_exploration(&mut self, dt: f32) {
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

    /// Configure les paramètres d'animation pour un déplacement du joueur.
    pub(super) fn begin_move_animation(&mut self, from: Position, to: Position) {
        self.moving = true;
        self.move_progress = 0.0;
        self.move_from = from;
        self.move_to = to;
        self.player_anim.frame = 0;
        self.player_anim.timer = 0.0;
        let nframes = 3;
        self.player_anim.frame_duration = self.move_time / nframes as f32;
    }

    /// Passe en mode combat contre un ennemi ciblé et affiche un message.
    pub(super) fn engage_combat_with_enemy(&mut self, enemy_idx: usize, message: &str) {
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

    /// Met à jour l'état de combat courant s'il est actif.
    pub(super) fn update_combat_state(&mut self) {
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

    /// Fait progresser un combat tour par tour et gère ses transitions.
    pub(super) fn update_combat(&mut self, state: &mut CombatState) -> bool {
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
                if matches!(
                    result,
                    CombatResult::JoueurVainqueur | CombatResult::DoubleKo
                ) {
                    let enemy_idx = state.enemy_index();
                    if enemy_idx < world.enemies.len() {
                        world.enemies.remove(enemy_idx);
                        rebuild_positions =
                            Some(world.enemies().iter().map(|e| e.position()).collect());
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

    /// Collecte les touches utilisées pour déclencher les actions de combat.
    pub(super) fn collect_combat_keys(&self) -> Vec<KeyCode> {
        [KeyCode::A, KeyCode::D, KeyCode::F]
            .iter()
            .copied()
            .filter(|key| is_key_pressed(*key))
            .collect()
    }
}
