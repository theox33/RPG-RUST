use crate::types::{Combatant, Position, Stats, PLAYER_BASE_STATS};
use rand::Rng;

#[derive(Clone, Debug)]
pub struct Joueur {
    id: usize,
    stats: Stats,
    pos: Position,
}

impl Joueur {
    pub fn nouveau(id: usize, pos: Position) -> Self {
        Self {
            id,
            stats: PLAYER_BASE_STATS,
            pos,
        }
    }

    pub fn gain_vie_if_lucky(&mut self) -> Option<u32> {
        let mut rng = rand::thread_rng();
        if rng.gen_bool(0.5) {
            let gain = rng.gen_range(10..=40);
            let stats = self.stats_mut();
            stats.vie = stats.vie.saturating_add(gain).min(PLAYER_BASE_STATS.vie);
            Some(gain)
        } else {
            None
        }
    }
}

impl Combatant for Joueur {
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
