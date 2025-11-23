use rand::Rng;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

/// Vitesse de base (pas par seconde) pour une entité de vitesse 10.
/// Multipliez par `stats.vitesse as f32 / 10.0` pour obtenir la vitesse effective.
/// Réduisez cette constante pour rendre toutes les entités plus lentes.
// pub const ENTITY_SPEED: f32 = 0.5; // 0.5 pas/seconde à vitesse 10 (lent)

#[derive(Clone, Copy, Debug)]
pub struct Stats {
    pub vie: u32,
    pub attaque: u32,
    pub vitesse: u32,
}

impl Stats {
    /// Construit une nouvelle structure de statistiques avec les valeurs fournies.
    pub const fn new(vie: u32, attaque: u32, vitesse: u32) -> Self {
        Self {
            vie,
            attaque,
            vitesse,
        }
    }

    /// Indique si l'entité possédant ces statistiques est toujours en vie.
    pub fn est_vivant(&self) -> bool {
        self.vie > 0
    }

    /// Soustrait des dégâts en veillant à ne pas passer sous zéro.
    pub fn take_damage(&mut self, degats: u32) {
        self.vie = self.vie.saturating_sub(degats);
    }
}

/// Statistiques de base pour le joueur et les ennemis.
pub const PLAYER_BASE_STATS: Stats = Stats::new(120, 15, 10);
pub const ENEMY_BASE_STATS: Stats = Stats::new(100, 12, 8);

pub trait Combatant {
    /// Retourne l'identifiant unique du combattant.
    fn id(&self) -> usize;
    /// Retourne la position actuelle du combattant.
    fn position(&self) -> Position;
    /// Fournit un accès mutable à la position du combattant.
    fn position_mut(&mut self) -> &mut Position;
    /// Retourne les statistiques actuelles du combattant.
    fn stats(&self) -> &Stats;
    /// Fournit un accès mutable aux statistiques du combattant.
    fn stats_mut(&mut self) -> &mut Stats;

    /// Indique si le combattant possède encore des points de vie.
    fn est_vivant(&self) -> bool {
        self.stats().est_vivant()
    }

    /// Calcule les dégâts d'une attaque avec une légère variance aléatoire.
    fn attaque(&self) -> u32 {
        let base = self.stats().attaque as i32;
        let mut rng = rand::thread_rng();
        let delta = rng.gen_range(-5..=5);
        (base + delta).max(1) as u32
    }

    /// Retourne la caractéristique de vitesse du combattant.
    fn vitesse(&self) -> u32 {
        self.stats().vitesse
    }

    /// Applique des dégâts au combattant.
    fn inflige_degats(&mut self, degats: u32) {
        self.stats_mut().take_damage(degats);
    }
}
