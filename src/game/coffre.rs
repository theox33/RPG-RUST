use super::{GameTextures, TileType, WorldKind, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use crate::types::Position;
use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};

const CHEST_FRAME_DURATION: f32 = 0.12;
const CHEST_SCALE: f32 = 0.25;
const CHEST_FADE_DURATION: f32 = 0.8;
const PROMPT_WIDTH: f32 = 220.0;
const PROMPT_HEIGHT: f32 = 44.0;

#[derive(Clone)]
pub struct ChestState {
    position: Position,
    opened: bool,
    animating: bool,
    anim_frame: usize,
    anim_timer: f32,
    fading: bool,
    fade_timer: f32,
    removed: bool,
    reward_pending: bool,
}

impl ChestState {
    fn new(position: Position) -> Self {
        Self {
            position,
            opened: false,
            animating: false,
            anim_frame: 0,
            anim_timer: 0.0,
            fading: false,
            fade_timer: 0.0,
            removed: false,
            reward_pending: false,
        }
    }

    fn current_frame(&self, max_frame: usize) -> usize {
        if self.animating {
            self.anim_frame.min(max_frame)
        } else if self.opened {
            max_frame
        } else {
            0
        }
    }
}

pub struct ChestSystem {
    chests: Vec<ChestState>,
    cache: HashMap<WorldKind, Vec<ChestState>>,
    active: Option<usize>,
}

impl ChestSystem {
    pub fn new() -> Self {
        Self {
            chests: Vec::new(),
            cache: HashMap::new(),
            active: None,
        }
    }

    pub fn draw(&self, textures: &GameTextures, origin_x: f32, origin_y: f32) {
        if self.chests.is_empty() || textures.chest_frames.is_empty() {
            return;
        }
        let max_frame = textures.chest_frames.len().saturating_sub(1);
        for chest in &self.chests {
            if chest.removed {
                continue;
            }
            let frame_idx = chest
                .current_frame(max_frame)
                .min(textures.chest_frames.len().saturating_sub(1));
            let src = textures.chest_frames[frame_idx];
            let width = src.w * CHEST_SCALE;
            let height = src.h * CHEST_SCALE;
            let tile_origin_x = origin_x + chest.position.x as f32 * TILE_SIZE;
            let tile_origin_y = origin_y + chest.position.y as f32 * TILE_SIZE;
            let dest_x = tile_origin_x + (TILE_SIZE - width) * 0.5;
            let dest_y = tile_origin_y + TILE_SIZE - height;
            let alpha = if chest.fading {
                (1.0 - (chest.fade_timer / CHEST_FADE_DURATION)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            draw_texture_ex(
                &textures.chest_texture,
                dest_x,
                dest_y,
                Color::new(1.0, 1.0, 1.0, alpha),
                DrawTextureParams {
                    dest_size: Some(Vec2::new(width, height)),
                    source: Some(src),
                    ..Default::default()
                },
            );
        }
    }

    pub fn draw_prompt(&self) {
        if self.active.is_none() {
            return;
        }
        let rect = prompt_rect();
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::new(0.15, 0.15, 0.2, 0.75),
        );
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, WHITE);
        let label = "Ouvrir le coffre";
        let text_x = rect.x + 18.0;
        let text_y = rect.y + rect.h * 0.65;
        draw_text(label, text_x, text_y, 24.0, WHITE);
    }

    pub fn refresh_prompt(&mut self, player_pos: Option<Position>) {
        let Some(player_pos) = player_pos else {
            self.active = None;
            return;
        };
        let next = self.chests.iter().enumerate().find(|(_, chest)| {
            !chest.opened
                && !chest.animating
                && !chest.removed
                && is_adjacent(player_pos, chest.position)
        });
        self.active = next.map(|(idx, _)| idx);
    }

    pub fn clear_active(&mut self) {
        self.active = None;
    }

    pub fn handle_click(&mut self, click_pos: Vec2) -> Option<Position> {
        let Some(idx) = self.active else {
            return None;
        };
        if prompt_rect().contains(click_pos) {
            let position = self.chests.get(idx).map(|c| c.position);
            if self.open_chest(idx) {
                return position;
            }
        }
        None
    }

    pub fn update_animation(&mut self, dt: f32, frame_count: usize) {
        if dt <= 0.0 || self.chests.is_empty() || frame_count == 0 {
            return;
        }
        let max_frame = frame_count.saturating_sub(1);
        for chest in &mut self.chests {
            if !chest.animating {
                // Even if not animating, handle fade progression if active
            }
            if chest.removed {
                continue;
            }

            // Advance opening animation if playing
            if chest.animating {
                chest.anim_timer += dt;
                while chest.anim_timer >= CHEST_FRAME_DURATION {
                    chest.anim_timer -= CHEST_FRAME_DURATION;
                    if chest.anim_frame < max_frame {
                        chest.anim_frame += 1;
                    }
                    if chest.anim_frame >= max_frame {
                        chest.animating = false;
                        chest.opened = true;
                        // start fade-out immediately after fully opened
                        chest.fading = true;
                        chest.fade_timer = 0.0;
                        chest.reward_pending = true;
                        break;
                    }
                }
            }

            // If fading, advance fade timer and mark removed when done
            if chest.fading {
                chest.fade_timer += dt;
                if chest.fade_timer >= CHEST_FADE_DURATION {
                    chest.fading = false;
                    chest.removed = true;
                }
            }
        }
    }

    pub fn rebuild_from_tiles(&mut self, tiles: &[Vec<TileType>], world: WorldKind) {
        let positions = detect_chest_positions(tiles);
        if positions.is_empty() {
            self.chests.clear();
            self.active = None;
            return;
        }
        let target_positions: HashSet<(usize, usize)> =
            positions.iter().map(|pos| (pos.x, pos.y)).collect();
        if let Some(cached) = self.cache.get(&world).cloned() {
            let mut kept: Vec<ChestState> = cached
                .into_iter()
                .filter(|chest| target_positions.contains(&(chest.position.x, chest.position.y)))
                .collect();
            let existing: HashSet<(usize, usize)> =
                kept.iter().map(|c| (c.position.x, c.position.y)).collect();
            for pos in positions {
                if !existing.contains(&(pos.x, pos.y)) {
                    kept.push(ChestState::new(pos));
                }
            }
            self.chests = kept;
        } else {
            self.chests = positions.into_iter().map(ChestState::new).collect();
        }
        self.active = None;
    }

    pub fn store_world_snapshot(&mut self, world: WorldKind) {
        if !self.chests.is_empty() {
            self.cache.insert(world, self.chests.clone());
        }
    }

    pub fn collect_opened_rewards(&mut self) -> Vec<Position> {
        let mut rewards = Vec::new();
        for chest in &mut self.chests {
            if chest.reward_pending {
                chest.reward_pending = false;
                rewards.push(chest.position);
            }
        }
        rewards
    }

    fn open_chest(&mut self, idx: usize) -> bool {
        if let Some(chest) = self.chests.get_mut(idx) {
            if chest.opened || chest.animating {
                return false;
            }
            chest.animating = true;
            chest.anim_frame = 0;
            chest.anim_timer = 0.0;
            self.active = None;
            return true;
        }
        false
    }
}

fn detect_chest_positions(tiles: &[Vec<TileType>]) -> Vec<Position> {
    let mut chests = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if matches!(tiles[y][x], TileType::Coffre | TileType::VictoryChest) {
                chests.push(Position { x, y });
            }
        }
    }
    chests
}

fn is_adjacent(a: Position, b: Position) -> bool {
    let dx = a.x as isize - b.x as isize;
    let dy = a.y as isize - b.y as isize;
    dx.abs() + dy.abs() == 1
}

fn prompt_rect() -> Rect {
    let x = 20.0;
    let y = screen_height() - PROMPT_HEIGHT - 20.0;
    Rect::new(x, y, PROMPT_WIDTH, PROMPT_HEIGHT)
}
