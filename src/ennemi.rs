use crate::types::{Combatant, Position, Stats, ENEMY_BASE_STATS};
use rand::Rng;

#[derive(Clone, Debug)]
pub struct Ennemi {
    id: usize,
    stats: Stats,
    pos: Position,
    move_timer: f32,
    move_delay: f32,
}

impl Ennemi {
    pub fn nouveau(id: usize, pos: Position) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let delay = rng.gen_range(0.5..=3.0);
        Self {
            id,
            stats: ENEMY_BASE_STATS,
            pos,
            move_timer: 0.0,
            move_delay: delay,
        }
    }

    /// Met à jour le timer et retourne true si l'ennemi doit se déplacer.
    pub fn tick_timer(&mut self, dt: f32) -> bool {
        self.move_timer += dt;
        if self.move_timer >= self.move_delay {
            self.move_timer = 0.0;
            // Tirer un nouveau délai aléatoire pour le prochain déplacement
            let mut rng = rand::thread_rng();
            self.move_delay = rng.gen_range(0.5..=3.0);
            true
        } else {
            false
        }
    }

    pub fn random_dir<R: Rng>(&self, rng: &mut R) -> (isize, isize) {
        match rng.gen_range(0..4) {
            0 => (0, -1),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (1, 0),
        }
    }
}

impl Combatant for Ennemi {
    fn id(&self) -> usize {
        self.id
    }

    fn position(&self) -> Position {
        self.pos
    }

    fn position_mut(&mut self) -> &mut Position {
        &mut self.pos
    }

    fn stats(&self) -> &Stats {
        &self.stats
    }

    fn stats_mut(&mut self) -> &mut Stats {
        &mut self.stats
    }
}
