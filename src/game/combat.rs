use rand::Rng;
#[derive(Debug)]
pub struct DamagePopup {
    value: i32,
    timer: f32,
    color: Color,
    x: f32,
    y: f32,
}
use macroquad::prelude::{draw_text, draw_rectangle, DARKGRAY, LIGHTGRAY, RED, GREEN, Color};
// ...existing code...

// ...existing code...
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
    _damage_popups: Vec<DamagePopup>,
    pending_result: Option<CombatResult>,
    result_delay: f32,
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
            _damage_popups: Vec::new(),
            pending_result: None,
            result_delay: 0.0,
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

    pub fn update(&mut self, world: &mut World, input: &CombatInput, origin_x: f32, origin_y: f32) -> CombatResolution {
                // Nettoyage des popups
                self._damage_popups.retain(|p| p.timer > 0.0);
                for popup in &mut self._damage_popups {
                    popup.timer -= 1.0 / 60.0;
                    popup.y -= 0.5; // effet flottant
                }

                if let Some(result) = self.pending_result.clone() {
                    if self.result_delay > 0.0 {
                        self.result_delay -= 1.0 / 60.0;
                        return CombatResolution {
                            messages: Vec::new(),
                            transition: CombatTransition::Continue,
                        };
                    }
                    self.pending_result = None;
                    return CombatResolution {
                        messages: Vec::new(),
                        transition: CombatTransition::Terminer(result),
                    };
                }
        self.update_buttons(input.tile_size, input.world_height, origin_x, origin_y);
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
                                // Popup dégâts infligés à l'ennemi (vert)
                                let ex = origin_x + enemy.position().x as f32 * input.tile_size + input.tile_size * 0.5;
                                let ey = origin_y + enemy.position().y as f32 * input.tile_size - 12.0;
                                self._damage_popups.push(DamagePopup {
                                    value: damage as i32,
                                    timer: 0.8,
                                    color: GREEN,
                                    x: ex,
                                    y: ey,
                                });
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
                        let mut rng = rand::thread_rng();
                        let gain = rng.gen_range(10..=40);
                        let pos = player.position();
                        let stats = player.stats_mut();
                        stats.vie = stats
                            .vie
                            .saturating_add(gain)
                            .min(crate::types::PLAYER_BASE_STATS.vie);
                        messages.push(CombatMessage {
                            texte: format!(
                                "Vous récupérez {} PV en fouillant l'ennemi vaincu.",
                                gain
                            ),
                            duree: 1.6,
                        });
                        // Popup rose pour gain de vie
                        let px = origin_x + pos.x as f32 * input.tile_size + input.tile_size * 0.5;
                        let py = origin_y + pos.y as f32 * input.tile_size - 12.0;
                        let rose = Color::new(1.0, 0.4, 0.7, 1.0);
                        self._damage_popups.push(DamagePopup {
                            value: gain as i32,
                            timer: 1.0,
                            color: rose,
                            x: px,
                            y: py,
                        });
                        self.pending_result = Some(CombatResult::JoueurVainqueur);
                        self.result_delay = 0.8;
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
            if matches!(result, CombatResult::JoueurVainqueur) {
                return CombatResolution {
                    messages,
                    transition: CombatTransition::Continue,
                };
            }
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
                // Popup dégâts subis par le joueur (rouge)
                let px = origin_x + player.position().x as f32 * input.tile_size + input.tile_size * 0.5;
                let py = origin_y + player.position().y as f32 * input.tile_size - 12.0;
                self._damage_popups.push(DamagePopup {
                    value: -(damage as i32),
                    timer: 0.8,
                    color: RED,
                    x: px,
                    y: py,
                });
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

    pub fn draw_ui(&self, world: &World, tile_size: f32, origin_x: f32, origin_y: f32) {
                // Affichage des popups de dégâts
                for popup in &self._damage_popups {
                    let txt = if popup.value > 0 {
                        format!("+{}", popup.value)
                    } else {
                        format!("{}", popup.value)
                    };
                    draw_text(&txt, popup.x - 12.0, popup.y, 22.0, popup.color);
                }
        let p = &world.players()[self.player_idx];
        let e = &world.enemies()[self.enemy_idx];
        let status = format!("Joueur PV: {}   Ennemi PV: {}", p.stats().vie, e.stats().vie);
        let margin = 16.0;
        let btn_w = 160.0;
        let btn_h = 36.0;
        let spacing = 12.0;
        let base_x = origin_x + margin;
        // Position sous la matrice graphique
        let base_y = origin_y + world.height as f32 * tile_size + margin;

        // Affichage PV
        draw_text(&status, base_x, base_y + 8.0, 22.0, DARKGRAY);

        // Boutons
        let btn_y = base_y + 36.0;
        let btn1_x = base_x;
        let btn2_x = base_x + btn_w + spacing;
        let btn3_x = base_x + 2.0 * (btn_w + spacing);

        draw_rectangle(btn1_x, btn_y, btn_w, btn_h, LIGHTGRAY);
        draw_rectangle(btn2_x, btn_y, btn_w, btn_h, LIGHTGRAY);
        draw_rectangle(btn3_x, btn_y, btn_w, btn_h, LIGHTGRAY);

        draw_text("Attaquer", btn1_x + 18.0, btn_y + 24.0, 20.0, DARKGRAY);
        draw_text("Défendre", btn2_x + 18.0, btn_y + 24.0, 20.0, DARKGRAY);
        draw_text("Fuir", btn3_x + 18.0, btn_y + 24.0, 20.0, DARKGRAY);
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

    fn update_buttons(&mut self, tile_size: f32, world_height: usize, origin_x: f32, origin_y: f32) {
        let margin = 16.0;
        let btn_w = 160.0;
        let btn_h = 36.0;
        let spacing = 12.0;
        let base_x = origin_x + margin;
        let base_y = origin_y + world_height as f32 * tile_size + margin + 36.0;

        self.boutons = CombatButtons {
            attaquer: Rect::new(base_x, base_y, btn_w, btn_h),
            defendre: Rect::new(base_x + (btn_w + spacing), base_y, btn_w, btn_h),
            fuir: Rect::new(base_x + 2.0 * (btn_w + spacing), base_y, btn_w, btn_h),
        };
    }
}
