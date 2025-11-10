use crate::entity::{Combatant, Personnage, Position};
use rand::thread_rng;
use std::collections::HashSet;

pub struct World {
    pub width: usize,
    pub height: usize,
    pub players: Vec<Personnage>,
    pub enemies: Vec<Personnage>,
}

impl World {
    pub fn new(width: usize, height: usize) -> Self {
        World { width, height, players: Vec::new(), enemies: Vec::new() }
    }

    pub fn add_player(&mut self, p: Personnage) {
        self.players.push(p);
    }

    pub fn add_enemy(&mut self, e: Personnage) {
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
            if !self.players[idx].est_vivant() { return false; }
            let current = self.players[idx].position();
            let nx = current.x as isize + dx;
            let ny = current.y as isize + dy;
            if self.is_within(nx, ny) {
                let (nxu, nyu) = (nx as usize, ny as usize);
                let can_move = (nxu == current.x && nyu == current.y) || self.is_cell_free(nxu, nyu);
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

    pub fn step_enemies(&mut self) {
        let players_snapshot: Vec<_> = self.players.iter().filter(|p| p.est_vivant()).map(|p| (p.id(), p.position())).collect();

        let mut occupied: HashSet<(usize, usize)> = HashSet::new();
        for (_, pos) in &players_snapshot { occupied.insert((pos.x, pos.y)); }
        for e in &self.enemies {
            if e.est_vivant() {
                let pos = e.position();
                occupied.insert((pos.x, pos.y));
            }
        }

        let mut new_positions: Vec<Option<Position>> = vec![None; self.enemies.len()];

        for (i, enemy) in self.enemies.iter().enumerate() {
            if !enemy.est_vivant() { continue; }
            if let Some((_, target_pos)) = players_snapshot.iter().min_by_key(|(_, pos)| {
                let dx = (pos.x as isize - enemy.position().x as isize).abs();
                let dy = (pos.y as isize - enemy.position().y as isize).abs();
                (dx + dy) as usize
            }) {
                let dx = target_pos.x as isize - enemy.position().x as isize;
                let dy = target_pos.y as isize - enemy.position().y as isize;
                let step_x = if dx == 0 { 0 } else if dx > 0 { 1 } else { -1 };
                let step_y = if dy == 0 { 0 } else if dy > 0 { 1 } else { -1 };

                let try_moves = [(step_x, 0), (0, step_y)];
                for (sx, sy) in try_moves.iter() {
                    let nx = enemy.position().x as isize + sx;
                    let ny = enemy.position().y as isize + sy;
                    if self.is_within(nx, ny) {
                        let (nxu, nyu) = (nx as usize, ny as usize);
                        if !occupied.contains(&(nxu, nyu)) {
                            occupied.insert((nxu, nyu));
                            new_positions[i] = Some(Position { x: nxu, y: nyu });
                            break;
                        }
                    }
                }
            }
        }

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

    /// Met à jour les ennemis avec un déplacement aléatoire lent basé sur un timer par entité.
    /// Utilise `dt` (en secondes) comme intervalle écoulé depuis le dernier tick.
    /// Évite les collisions avec les joueurs et entre ennemis.
    pub fn wander_enemies(&mut self, dt: f32) {
        if dt <= 0.0 { return; }

        // Snapshot des positions des joueurs vivants
        let players_snapshot: Vec<_> = self
            .players
            .iter()
            .filter(|p| p.est_vivant())
            .map(|p| (p.id(), p.position()))
            .collect();

        // Occupation initiale
        let mut occupied = std::collections::HashSet::<(usize, usize)>::new();
        for (_, pos) in &players_snapshot { occupied.insert((pos.x, pos.y)); }
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
            if !enemy.est_vivant() { continue; }

            // Combien de pas sont dus avec ce dt ? On ne tente qu'un pas par tick.
            let steps = enemy.steps_due(dt);
            if steps == 0 { continue; }

            let current = enemy.position();
            // Essayer jusqu'à 4 directions aléatoires pour trouver une case libre
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

    pub fn any_adjacent(&self) -> bool {
        self.find_adjacent_pair().is_some()
    }

    pub fn find_adjacent_pair(&self) -> Option<(usize, usize)> {
        for (pi, p) in self.players.iter().enumerate() {
            if !p.est_vivant() { continue; }
            for (ei, e) in self.enemies.iter().enumerate() {
                if !e.est_vivant() { continue; }
                let p_pos = p.position();
                let e_pos = e.position();
                let dist = (p_pos.x as isize - e_pos.x as isize).abs() + (p_pos.y as isize - e_pos.y as isize).abs();
                if dist == 1 { return Some((pi, ei)); }
            }
        }
        None
    }

    pub fn players(&self) -> &[Personnage] {
        &self.players
    }

    pub fn enemies(&self) -> &[Personnage] {
        &self.enemies
    }

    pub fn players_mut(&mut self) -> &mut [Personnage] {
        &mut self.players
    }

    pub fn enemies_mut(&mut self) -> &mut [Personnage] {
        &mut self.enemies
    }
}
