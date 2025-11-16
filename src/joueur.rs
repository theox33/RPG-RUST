use crate::types::{Combatant, Position, Stats, PLAYER_BASE_STATS};

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
