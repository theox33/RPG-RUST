use super::map_utils::{
    build_walkable_map, choose_chemin_variants, choose_grass_variants, default_map_tiles,
    detect_house_anchors, enemy_cap, find_tile_position, load_tiles_for_world,
    spawn_random_enemies,
};
use super::*;

impl Game {
    pub(super) fn refresh_map_variants(&mut self) {
        self.grass_choice = choose_grass_variants(&self.textures, &self.map_tiles);
        self.chemin_choice = choose_chemin_variants(&self.textures, &self.map_tiles);
        self.house_anchors = detect_house_anchors(&self.map_tiles);
    }

    pub(super) fn rebuild_chests_from_tiles(&mut self) {
        self.chests
            .rebuild_from_tiles(&self.map_tiles, self.current_world);
    }

    pub(super) fn sync_world_walkable_map(&self) {
        if let Ok(mut world) = self.world.lock() {
            world.update_walkable_map(build_walkable_map(&self.map_tiles));
        }
    }

    pub(super) fn respawn_enemies_for_current_world(&mut self) {
        let mut snapshot = Vec::new();
        if let Ok(mut world) = self.world.lock() {
            world.enemies.clear();
            if let Some(stored) = self.enemy_cache.get(&self.current_world).cloned() {
                snapshot = stored.iter().map(|e| e.position()).collect();
                world.enemies = stored;
            } else {
                let cap = enemy_cap(self.current_world);
                if cap > 0 {
                    spawn_random_enemies(&mut world, &self.map_tiles, cap, self.current_world);
                }
                snapshot = world.enemies().iter().map(|e| e.position()).collect();
            }
        }
        self.rebuild_enemy_animation_state(snapshot);
    }

    pub(super) fn store_current_world_enemies(&mut self) {
        if let Ok(world) = self.world.lock() {
            self.enemy_cache
                .insert(self.current_world, world.enemies().to_vec());
        }
    }

    pub(super) fn rebuild_enemy_animation_state(&mut self, positions: Vec<Position>) {
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

    pub(super) fn move_player_to(&mut self, pos: Position) {
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

    pub(super) fn trigger_portal_transition(&mut self) {
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

    pub(super) fn trigger_spiral_transition(&mut self) {
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

    pub(super) fn trigger_spiral2_transition(&mut self) {
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

    pub(super) fn switch_to_world(&mut self, target: WorldKind, spawn_tile: TileType) -> bool {
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

    pub(super) fn store_current_world_chests(&mut self) {
        self.chests.store_world_snapshot(self.current_world);
    }

    pub(super) fn apply_player_attack_bonus(&mut self) {
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

    pub(super) fn trigger_victory(&mut self) {
        if matches!(
            self.state,
            GameState::GameOver(GameOverState {
                allow_restart: false,
                ..
            })
        ) {
            return;
        }
        self.chests.clear_active();
        self.state = GameState::GameOver(GameOverState {
            selected: 1,
            allow_restart: false,
        });
    }
}
