use crate::types::Combatant;
use crate::world::World;
use macroquad::prelude::{KeyCode, Rect, Vec2};

#[derive(Clone, Debug)]
pub enum CombatResult {
    JoueurVainqueur,
    EnnemiVainqueur,
    DoubleKo,
    Fuite,
}

#[derive(Clone, Debug)]
pub struct CombatMessage {
    pub texte: String,
    pub duree: f32,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct CombatButtons {
    pub attaquer: Rect,
    pub defendre: Rect,
    pub fuir: Rect,
}

#[derive(Debug)]
pub struct CombatState {
    player_idx: usize,
    enemy_idx: usize,
    player_defending: bool,
    enemy_defending: bool,
    player_turn: bool,
    boutons: CombatButtons,
}

pub struct CombatInput {
    pub keys_pressed: Vec<KeyCode>,
    pub mouse_click: Option<Vec2>,
    pub tile_size: f32,
    pub world_height: usize,
}

pub struct CombatResolution {
    pub messages: Vec<CombatMessage>,
    pub transition: CombatTransition,
}

pub enum CombatTransition {
    Continue,
    Terminer(CombatResult),
}

enum PlayerAction {
    Attack,
    Defend,
    Flee,
}

impl CombatState {
    pub fn new(player_idx: usize, enemy_idx: usize) -> Self {
        Self {
            player_idx,
            enemy_idx,
            player_defending: false,
            enemy_defending: false,
            player_turn: true,
            boutons: CombatButtons::default(),
        }
    }

    pub fn with_initiative(player_idx: usize, enemy_idx: usize, player_first: bool) -> Self {
        let mut state = Self::new(player_idx, enemy_idx);
        state.player_turn = player_first;
        state
    }

    pub fn enemy_index(&self) -> usize {
        self.enemy_idx
    }

    pub fn update(&mut self, world: &mut World, input: &CombatInput) -> CombatResolution {
        self.update_buttons(input.tile_size, input.world_height);
        let mut messages = Vec::new();

        // Vérifier que les indices restent valides
        if self.player_idx >= world.players().len() || self.enemy_idx >= world.enemies().len() {
            return CombatResolution {
                messages,
                transition: CombatTransition::Terminer(CombatResult::DoubleKo),
            };
        }

        if self.player_turn {
            let player_alive = world.players()[self.player_idx].est_vivant();
            if player_alive {
                if let Some(action) = self.extract_player_action(input) {
                    match action {
                        PlayerAction::Attack => {
                            let base_dmg = world.players()[self.player_idx].attaque();
                            let damage = if self.enemy_defending {
                                base_dmg / 2
                            } else {
                                base_dmg
                            };
                            if let Some(enemy) = world.enemies_mut().get_mut(self.enemy_idx) {
                                enemy.inflige_degats(damage);
                            }
                            self.enemy_defending = false;
                            self.player_turn = false;
                            messages.push(CombatMessage {
                                texte: format!("Vous attaquez et infligez {} dégâts", damage),
                                duree: 1.0,
                            });
                        }
                        PlayerAction::Defend => {
                            self.player_defending = true;
                            self.player_turn = false;
                            messages.push(CombatMessage {
                                texte: "Vous vous préparez à encaisser (dégâts réduits)"
                                    .to_string(),
                                duree: 0.8,
                            });
                        }
                        PlayerAction::Flee => {
                            let ps = world.players()[self.player_idx].vitesse();
                            let es = world.enemies()[self.enemy_idx].vitesse();
                            let réussite = if ps > es {
                                true
                            } else {
                                macroquad::rand::gen_range(0, 100) < 30
                            };
                            if réussite {
                                let player_id = world.players()[self.player_idx].id();
                                let _ = world.move_player(player_id, -1, 0);
                                messages.push(CombatMessage {
                                    texte: "Vous prenez la fuite !".to_string(),
                                    duree: 1.2,
                                });
                                return CombatResolution {
                                    messages,
                                    transition: CombatTransition::Terminer(CombatResult::Fuite),
                                };
                            } else {
                                self.player_turn = false;
                                messages.push(CombatMessage {
                                    texte: "Vous n'arrivez pas à fuir...".to_string(),
                                    duree: 1.0,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Vérifier conditions après l'action du joueur
        let player_alive = world.players()[self.player_idx].est_vivant();
        let enemy_alive = world.enemies()[self.enemy_idx].est_vivant();
        if !player_alive || !enemy_alive {
            let result = match (player_alive, enemy_alive) {
                (true, false) => {
                    if let Some(player) = world.players_mut().get_mut(self.player_idx) {
                        if let Some(gain) = player.gain_vie_if_lucky() {
                            messages.push(CombatMessage {
                                texte: format!(
                                    "Vous récupérez {} PV en fouillant l'ennemi vaincu.",
                                    gain
                                ),
                                duree: 1.6,
                            });
                        }
                    }
                    CombatResult::JoueurVainqueur
                }
                (false, true) => CombatResult::EnnemiVainqueur,
                (false, false) => CombatResult::DoubleKo,
                _ => CombatResult::DoubleKo,
            };
            let texte = match result {
                CombatResult::JoueurVainqueur => "Vous avez vaincu l'ennemi !".to_string(),
                CombatResult::EnnemiVainqueur => "Vous succombez à vos blessures...".to_string(),
                CombatResult::DoubleKo => "Les deux combattants tombent en même temps.".to_string(),
                CombatResult::Fuite => unreachable!(),
            };
            messages.push(CombatMessage {
                texte: texte.clone(),
                duree: 2.0,
            });
            return CombatResolution {
                messages,
                transition: CombatTransition::Terminer(result),
            };
        }

        if !self.player_turn {
            let base_dmg = world.enemies()[self.enemy_idx].attaque();
            let damage = if self.player_defending {
                base_dmg / 2
            } else {
                base_dmg
            };
            if let Some(player) = world.players_mut().get_mut(self.player_idx) {
                player.inflige_degats(damage);
            }
            self.player_defending = false;
            self.player_turn = true;
            messages.push(CombatMessage {
                texte: format!("L'ennemi inflige {} dégâts", damage),
                duree: 1.0,
            });
        }

        // Vérifier après l'attaque de l'ennemi
        let player_alive = world.players()[self.player_idx].est_vivant();
        let enemy_alive = world.enemies()[self.enemy_idx].est_vivant();
        if !player_alive || !enemy_alive {
            let result = match (player_alive, enemy_alive) {
                (true, false) => {
                    if let Some(player) = world.players_mut().get_mut(self.player_idx) {
                        if let Some(gain) = player.gain_vie_if_lucky() {
                            messages.push(CombatMessage {
                                texte: format!(
                                    "Vous récupérez {} PV en fouillant l'ennemi vaincu.",
                                    gain
                                ),
                                duree: 1.6,
                            });
                        }
                    }
                    CombatResult::JoueurVainqueur
                }
                (false, true) => CombatResult::EnnemiVainqueur,
                (false, false) => CombatResult::DoubleKo,
                _ => CombatResult::DoubleKo,
            };
            let texte = match result {
                CombatResult::JoueurVainqueur => "Vous avez vaincu l'ennemi !".to_string(),
                CombatResult::EnnemiVainqueur => "Vous succombez à vos blessures...".to_string(),
                CombatResult::DoubleKo => "Les deux combattants tombent en même temps.".to_string(),
                CombatResult::Fuite => unreachable!(),
            };
            messages.push(CombatMessage {
                texte: texte.clone(),
                duree: 2.0,
            });
            return CombatResolution {
                messages,
                transition: CombatTransition::Terminer(result),
            };
        }

        CombatResolution {
            messages,
            transition: CombatTransition::Continue,
        }
    }

    pub fn draw_ui(&self, world: &World, tile_size: f32) {
        let p = &world.players()[self.player_idx];
        let e = &world.enemies()[self.enemy_idx];
        let status = format!(
            "Joueur HP: {}   Ennemi HP: {}",
            p.stats().vie,
            e.stats().vie
        );
        let base_y = (world.height as f32) * tile_size + 20.0;
        macroquad::prelude::draw_text(&status, 10.0, base_y, 20.0, macroquad::prelude::DARKGRAY);

        let buttons = self.boutons;
        macroquad::prelude::draw_rectangle(
            buttons.attaquer.x,
            buttons.attaquer.y,
            buttons.attaquer.w,
            buttons.attaquer.h,
            macroquad::prelude::LIGHTGRAY,
        );
        macroquad::prelude::draw_rectangle(
            buttons.defendre.x,
            buttons.defendre.y,
            buttons.defendre.w,
            buttons.defendre.h,
            macroquad::prelude::LIGHTGRAY,
        );
        macroquad::prelude::draw_rectangle(
            buttons.fuir.x,
            buttons.fuir.y,
            buttons.fuir.w,
            buttons.fuir.h,
            macroquad::prelude::LIGHTGRAY,
        );

        macroquad::prelude::draw_text(
            "Attaquer",
            buttons.attaquer.x + 12.0,
            buttons.attaquer.y + 24.0,
            20.0,
            macroquad::prelude::DARKGRAY,
        );
        macroquad::prelude::draw_text(
            "Défendre",
            buttons.defendre.x + 12.0,
            buttons.defendre.y + 24.0,
            20.0,
            macroquad::prelude::DARKGRAY,
        );
        macroquad::prelude::draw_text(
            "Fuir",
            buttons.fuir.x + 12.0,
            buttons.fuir.y + 24.0,
            20.0,
            macroquad::prelude::DARKGRAY,
        );
    }

    fn extract_player_action(&self, input: &CombatInput) -> Option<PlayerAction> {
        if let Some(click) = input.mouse_click {
            if self.boutons.attaquer.contains(click) {
                return Some(PlayerAction::Attack);
            }
            if self.boutons.defendre.contains(click) {
                return Some(PlayerAction::Defend);
            }
            if self.boutons.fuir.contains(click) {
                return Some(PlayerAction::Flee);
            }
        }

        for key in &input.keys_pressed {
            match key {
                KeyCode::A => return Some(PlayerAction::Attack),
                KeyCode::D => return Some(PlayerAction::Defend),
                KeyCode::F => return Some(PlayerAction::Flee),
                _ => {}
            }
        }
        None
    }

    fn update_buttons(&mut self, tile_size: f32, world_height: usize) {
        let base_y = world_height as f32 * tile_size + 40.0;
        let btn_w = 160.0;
        let btn_h = 36.0;
        let spacing = 12.0;
        let bx = 10.0;

        self.boutons = CombatButtons {
            attaquer: Rect::new(bx, base_y, btn_w, btn_h),
            defendre: Rect::new(bx + (btn_w + spacing), base_y, btn_w, btn_h),
            fuir: Rect::new(bx + 2.0 * (btn_w + spacing), base_y, btn_w, btn_h),
        };
    }
}
