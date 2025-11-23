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
    walkable: Vec<Vec<bool>>,
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
    /// Crée un monde rectangulaire vide de dimensions données.
    pub fn new(width: usize, height: usize) -> Self {
        World {
            width,
            height,
            players: Vec::new(),
            enemies: Vec::new(),
            enemies_frozen: false,
            walkable: vec![vec![true; width]; height],
        }
    }

    /// Ajoute un joueur dans la liste interne.
    pub fn add_player(&mut self, p: Joueur) {
        self.players.push(p);
    }

    /// Ajoute un ennemi dans la liste interne.
    pub fn add_enemy(&mut self, e: Ennemi) {
        self.enemies.push(e);
    }

    /// Vérifie si une coordonnée appartient au monde.
    pub fn is_within(&self, x: isize, y: isize) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height
    }

    /// Indique si une cellule donnée est libre (aucun joueur ou ennemi vivant).
    fn is_cell_free(&self, x: usize, y: usize) -> bool {
        self.players.iter().filter(|p| p.est_vivant()).all(|p| {
            let pos = p.position();
            !(pos.x == x && pos.y == y)
        }) && self.enemies.iter().filter(|e| e.est_vivant()).all(|e| {
            let pos = e.position();
            !(pos.x == x && pos.y == y)
        })
    }

    /// Déplace un joueur identifié d'un vecteur (dx, dy) si la case est libre.
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
    /// Évite les collisions entre ennemis, mais leur permet d'atteindre la case du joueur.
    pub fn wander_enemies(&mut self, dt: f32) {
        if dt <= 0.0 || self.enemies_frozen {
            return;
        }

        // Occupation initiale : on évite les collisions entre ennemis, mais pas avec le joueur.
        let mut enemy_occupied: HashSet<(usize, usize)> = HashSet::new();
        for e in &self.enemies {
            if e.est_vivant() {
                let pos = e.position();
                enemy_occupied.insert((pos.x, pos.y));
            }
        }

        let mut rng = thread_rng();
        let w = self.width as isize;
        let h = self.height as isize;
        let walkable_map = self.walkable.clone();
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
                    if walkable_map
                        .get(nyu)
                        .and_then(|row| row.get(nxu))
                        .copied()
                        .unwrap_or(false)
                        && !enemy_occupied.contains(&(nxu, nyu))
                    {
                        moved_to = Some((nxu, nyu));
                        break;
                    }
                }
            }

            if let Some((nxu, nyu)) = moved_to {
                enemy_occupied.remove(&(current.x, current.y));
                enemy_occupied.insert((nxu, nyu));
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

    /// Retourne une vue immuable sur les joueurs.
    pub fn players(&self) -> &[Joueur] {
        &self.players
    }

    /// Retourne une vue immuable sur les ennemis.
    pub fn enemies(&self) -> &[Ennemi] {
        &self.enemies
    }

    /// Retourne une vue mutable sur les joueurs.
    pub fn players_mut(&mut self) -> &mut [Joueur] {
        &mut self.players
    }

    /// Retourne une vue mutable sur les ennemis.
    pub fn enemies_mut(&mut self) -> &mut [Ennemi] {
        &mut self.enemies
    }

    /// Met à jour la carte de traversabilité utilisée par l'IA.
    pub fn update_walkable_map(&mut self, walkable: Vec<Vec<bool>>) {
        if walkable.len() == self.height && walkable.iter().all(|row| row.len() == self.width) {
            self.walkable = walkable;
        }
    }
}
