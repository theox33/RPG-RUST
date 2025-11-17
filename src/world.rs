use crate::ennemi::Ennemi;
use crate::joueur::Joueur;
use crate::types::{Combatant, Position};
use ::rand::thread_rng;
use std::collections::HashSet;

pub struct World {
    pub width: usize,
    pub height: usize,
    pub players: Vec<Joueur>,
    pub enemies: Vec<Ennemi>,
    pub enemies_frozen: bool, // True si les ennemis sont figés (combat)
}

impl World {
    /// Retourne l'indice d'un ennemi vivant sur la case (x, y), ou None
    pub fn find_enemy_on_tile(&self, x: usize, y: usize) -> Option<usize> {
        self.enemies.iter().enumerate().find_map(|(ei, e)| {
            if e.est_vivant() && e.position().x == x && e.position().y == y {
                Some(ei)
            } else {
                None
            }
        })
    }
    pub fn new(width: usize, height: usize) -> Self {
        World {
            width,
            height,
            players: Vec::new(),
            enemies: Vec::new(),
            enemies_frozen: false,
        }
    }

    pub fn add_player(&mut self, p: Joueur) {
        self.players.push(p);
    }

    pub fn add_enemy(&mut self, e: Ennemi) {
        self.enemies.push(e);
    }

    pub fn is_within(&self, x: isize, y: isize) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    fn is_cell_free(&self, x: usize, y: usize) -> bool {
        self.players.iter().filter(|p| p.est_vivant()).all(|p| {
            let pos = p.position();
            !(pos.x == x && pos.y == y)
        }) && self.enemies.iter().filter(|e| e.est_vivant()).all(|e| {
            let pos = e.position();
            !(pos.x == x && pos.y == y)
        })
    }

    pub fn move_player(&mut self, player_id: usize, dx: isize, dy: isize) -> bool {
        if let Some(idx) = self.players.iter().position(|p| p.id() == player_id) {
            if !self.players[idx].est_vivant() {
                return false;
            }
            let current = self.players[idx].position();
            let nx = current.x as isize + dx;
            let ny = current.y as isize + dy;
            if self.is_within(nx, ny) {
                let (nxu, nyu) = (nx as usize, ny as usize);
                let can_move =
                    (nxu == current.x && nyu == current.y) || self.is_cell_free(nxu, nyu);
                if can_move {
                    if let Some(p) = self.players.get_mut(idx) {
                        let pos = p.position_mut();
                        pos.x = nxu;
                        pos.y = nyu;
                    }
                    return true;
                }
            }
        }
        false
    }

    /// Met à jour les ennemis avec un déplacement aléatoire lent basé sur un timer par entité.
    /// Utilise `dt` (en secondes) comme intervalle écoulé depuis le dernier tick.
    /// Évite les collisions avec les joueurs et entre ennemis.
    pub fn wander_enemies(&mut self, dt: f32) {
        if dt <= 0.0 || self.enemies_frozen {
            return;
        }

        // Snapshot des positions des joueurs vivants
        let players_snapshot: Vec<_> = self
            .players
            .iter()
            .filter(|p| p.est_vivant())
            .map(|p| (p.id(), p.position()))
            .collect();

        // Occupation initiale
        let mut occupied: HashSet<(usize, usize)> = HashSet::new();
        for (_, pos) in &players_snapshot {
            occupied.insert((pos.x, pos.y));
        }
        for e in &self.enemies {
            if e.est_vivant() {
                let pos = e.position();
                occupied.insert((pos.x, pos.y));
            }
        }

        let mut rng = thread_rng();
        let w = self.width as isize;
        let h = self.height as isize;
        let mut new_positions: Vec<Option<Position>> = vec![None; self.enemies.len()];

        for (i, enemy) in self.enemies.iter_mut().enumerate() {
            if !enemy.est_vivant() {
                continue;
            }

            // Timer aléatoire : l'ennemi ne bouge que si son timer est écoulé
            if !enemy.tick_timer(dt) {
                continue;
            }

            let current = enemy.position();
            let mut moved_to: Option<(usize, usize)> = None;
            for _ in 0..4 {
                let (dx, dy) = enemy.random_dir(&mut rng);
                let nx = current.x as isize + dx;
                let ny = current.y as isize + dy;
                if nx >= 0 && ny >= 0 && nx < w && ny < h {
                    let (nxu, nyu) = (nx as usize, ny as usize);
                    if !occupied.contains(&(nxu, nyu)) {
                        moved_to = Some((nxu, nyu));
                        break;
                    }
                }
            }

            if let Some((nxu, nyu)) = moved_to {
                occupied.remove(&(current.x, current.y));
                occupied.insert((nxu, nyu));
                new_positions[i] = Some(Position { x: nxu, y: nyu });
            }
        }

        // Appliquer les nouvelles positions
        for (i, pos_opt) in new_positions.into_iter().enumerate() {
            if let Some(pos) = pos_opt {
                if let Some(enemy) = self.enemies.get_mut(i) {
                    let enemy_pos = enemy.position_mut();
                    enemy_pos.x = pos.x;
                    enemy_pos.y = pos.y;
                }
            }
        }
    }

    pub fn players(&self) -> &[Joueur] {
        &self.players
    }

    pub fn enemies(&self) -> &[Ennemi] {
        &self.enemies
    }

    pub fn players_mut(&mut self) -> &mut [Joueur] {
        &mut self.players
    }

    pub fn enemies_mut(&mut self) -> &mut [Ennemi] {
        &mut self.enemies
    }
}
