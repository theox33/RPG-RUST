use macroquad::prelude::*;

use super::*;

impl Game {
    /// Dessine l'ensemble de la scène (monde, entités, UI) pour le frame courant.
    pub(super) fn render(&mut self) {
        let (origin_x, origin_y) = self.world_origin();
        self.draw_tiles(origin_x, origin_y);
        self.chests.draw(&self.textures, origin_x, origin_y);
        if matches!(self.current_world, WorldKind::Plaine) {
            self.draw_houses(origin_x, origin_y);
        }
        if let Ok(world) = self.world.lock() {
            match &self.state {
                GameState::GameOver(_) => {}
                GameState::Combat(state) => {
                    self.draw_player(&world, origin_x, origin_y);
                    self.draw_enemies(&world, origin_x, origin_y);
                    let enemy = &world.enemies()[state.enemy_index()];
                    let ex = origin_x + enemy.position().x as f32 * TILE_SIZE;
                    let ey = origin_y + enemy.position().y as f32 * TILE_SIZE;
                    let bar_w = 48.0;
                    let bar_h = 8.0;
                    let bar_x = ex + (TILE_SIZE - bar_w) * 0.5;
                    let bar_y = ey - bar_h - 6.0;
                    let max_vie = if matches!(self.current_world, WorldKind::Spirale2) {
                        200
                    } else {
                        100
                    };
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
            self.draw_enemy_counter();
        }
    }

    /// Dessine la barre de vie du joueur à l'emplacement approprié.
    pub(super) fn draw_health_bar(&self) {
        let (origin_x, origin_y) = self.world_origin();
        if let Ok(world) = self.world.lock() {
            if let Some(player) = world.players().get(0) {
                self.health_bar.draw_at(player.stats(), origin_x, origin_y);
            }
        }
    }

    /// Calcule le décalage nécessaire pour centrer le monde à l'écran.
    pub(super) fn world_origin(&self) -> (f32, f32) {
        let world_w = MAP_WIDTH as f32 * TILE_SIZE;
        let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
        let origin_x = ((screen_width() - world_w) * 0.5).max(0.0);
        let origin_y = ((screen_height() - world_h) * 0.5).max(0.0);
        (origin_x, origin_y)
    }

    /// Dessine les tuiles de terrain à partir des textures sélectionnées.
    pub(super) fn draw_tiles(&self, origin_x: f32, origin_y: f32) {
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
                        if !self.textures.chemin_variants.is_empty() {
                            self.textures.chemin_variants[0]
                        } else {
                            self.textures.chemin_src
                        }
                    }
                    TileType::CollisionInvisible | TileType::Coffre | TileType::VictoryChest => {
                        Rect::new(0.0, 0.0, 0.0, 0.0)
                    }
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

    /// Affiche l'arrière-plan spécifique à l'intérieur de la maison.
    pub(super) fn draw_maison_background(&self, origin_x: f32, origin_y: f32) {
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

    /// Dessine le sprite du joueur en tenant compte des interpolations de mouvement.
    pub(super) fn draw_player(&self, world: &World, origin_x: f32, origin_y: f32) {
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

    /// Dessine chaque ennemi avec son animation et son interpolation propres.
    pub(super) fn draw_enemies(&self, world: &World, origin_x: f32, origin_y: f32) {
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

    /// Affiche tous les messages temporaires, centrés ou non.
    pub(super) fn draw_messages(&self) {
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
                let offset =
                    (idx as f32 - (total as f32 - 1.0) * 0.5) * (box_height + stack_spacing);
                self.draw_centered_message(&msg.texte, offset);
            }
        }
    }

    /// Dessine un message centré avec un fond semi-transparent.
    pub(super) fn draw_centered_message(&self, texte: &str, offset_y: f32) {
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

    /// Dessine les maisons statiques disposées sur la carte.
    pub(super) fn draw_houses(&self, origin_x: f32, origin_y: f32) {
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

    /// Affiche le compteur d'ennemis restants dans le coin supérieur droit.
    pub(super) fn draw_enemy_counter(&self) {
        let remaining = self.remaining_enemies_total();
        let label = format!("Ennemis restants : {}", remaining);
        let font_size = 24.0;
        let dims = measure_text(&label, None, font_size as u16, 1.0);
        let padding = 12.0;
        let margin = 16.0;
        let rect_w = dims.width + padding * 2.0;
        let rect_h = dims.height + padding * 1.5;
        let rect_x = screen_width() - rect_w - margin;
        let rect_y = margin;
        draw_rectangle(
            rect_x,
            rect_y,
            rect_w,
            rect_h,
            Color::new(0.0, 0.0, 0.0, 0.65),
        );
        draw_rectangle_lines(rect_x, rect_y, rect_w, rect_h, 1.5, WHITE);
        let text_x = rect_x + padding;
        let text_y = rect_y + rect_h - padding * 0.4;
        draw_text(&label, text_x, text_y, font_size, WHITE);
    }
}
