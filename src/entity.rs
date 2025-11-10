use std::fmt;
use rand::Rng;

#[derive(Clone, Debug)]
pub enum Classe {
    Soldat,
    Magicien,
    Assassin,
}

impl fmt::Display for Classe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Classe::Soldat => "Soldat",
            Classe::Magicien => "Magicien",
            Classe::Assassin => "Assassin",
        };
        write!(f, "{}", s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

/// Vitesse de base (pas par seconde) pour une entité de vitesse 10.
/// Multipliez par `stats.vitesse as f32 / 10.0` pour obtenir la vitesse effective.
/// Réduisez cette constante pour rendre toutes les entités plus lentes.
pub const ENTITY_SPEED: f32 = 0.5; // 0.5 pas/seconde à vitesse 10 (lent)

#[derive(Clone, Debug)]
pub struct Stats {
    pub vie: u32,
    pub attaque: u32,
    pub vitesse: u32,
}

impl Stats {
    pub fn new(vie: u32, attaque: u32, vitesse: u32) -> Self {
        Self { vie, attaque, vitesse }
    }

    pub fn est_vivant(&self) -> bool {
        self.vie > 0
    }

    pub fn take_damage(&mut self, degats: u32) {
        self.vie = self.vie.saturating_sub(degats);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Joueur,
    Ennemi,
}

pub trait Combatant {
    fn id(&self) -> usize;
    fn classe(&self) -> Classe;
    fn faction(&self) -> Faction;
    fn position(&self) -> Position;
    fn position_mut(&mut self) -> &mut Position;
    fn stats(&self) -> &Stats;
    fn stats_mut(&mut self) -> &mut Stats;

    fn est_vivant(&self) -> bool {
        self.stats().est_vivant()
    }

    fn attaque(&self) -> u32 {
        self.stats().attaque
    }

    fn vitesse(&self) -> u32 {
        self.stats().vitesse
    }

    fn inflige_degats(&mut self, degats: u32) {
        self.stats_mut().take_damage(degats);
    }
}

#[derive(Clone, Debug)]
pub struct Personnage {
    id: usize,
    classe: Classe,
    faction: Faction,
    stats: Stats,
    pos: Position,
    /// Accumulateur de pas pour le déplacement (timer/cooldown continu)
    move_accum: f32,
}

impl Personnage {
    pub fn nouveau_joueur(id: usize, classe: Classe, pos: Position) -> Self {
        let stats = match classe {
            Classe::Soldat => Stats::new(120, 15, 10),
            Classe::Magicien => Stats::new(80, 25, 12),
            Classe::Assassin => Stats::new(90, 18, 20),
        };
        Self { id, classe, faction: Faction::Joueur, stats, pos, move_accum: 0.0 }
    }

    pub fn nouvel_ennemi(id: usize, classe: Classe, pos: Position) -> Self {
        let stats = match classe {
            Classe::Soldat => Stats::new(100, 12, 8),
            Classe::Magicien => Stats::new(60, 20, 10),
            Classe::Assassin => Stats::new(70, 14, 15),
        };
        Self { id, classe, faction: Faction::Ennemi, stats, pos, move_accum: 0.0 }
    }

    /// Vitesse effective en pas par seconde, tenant compte des stats.
    pub fn effective_speed(&self) -> f32 {
        let mult = (self.stats.vitesse as f32) / 10.0;
        ENTITY_SPEED * mult
    }

    /// Ajoute dt au timer et retourne le nombre de pas à exécuter (entier),
    /// en conservant la fraction restante dans l'accumulateur.
    pub fn steps_due(&mut self, dt: f32) -> usize {
        if dt <= 0.0 { return 0; }
        self.move_accum += self.effective_speed() * dt;
        let steps = self.move_accum.floor() as usize;
        self.move_accum -= steps as f32;
        steps
    }

    /// Retourne une direction aléatoire cardinale (dx, dy) où |dx|+|dy| = 1.
    pub fn random_dir<R: Rng>(&self, rng: &mut R) -> (isize, isize) {
        match rng.gen_range(0..4) {
            0 => (0, -1),
            1 => (0, 1),
            2 => (-1, 0),
            _ => (1, 0),
        }
    }

    /// Déplace l'entité de manière aléatoire en fonction de la vitesse `ENTITY_SPEED`.
    ///
    /// - `rng`: générateur aléatoire (ex: &mut rand::thread_rng()).
    /// - `dt`: delta time en secondes depuis la dernière mise à jour.
    /// - `max_x`, `max_y`: dimensions du monde (pour clamp des positions).
    pub fn update_random_movement<R: Rng>(&mut self, rng: &mut R, dt: f32, max_x: usize, max_y: usize) {
        if dt <= 0.0 {
            return;
        }

        // Nombre attendu de cellules à déplacer pendant dt (peut être fractional).
        let expected_moves = ENTITY_SPEED * dt;

        // Déplacement entier (floor) : exécution déterministe de ces pas.
        let steps = expected_moves.floor() as usize;
        for _ in 0..steps {
            Self::one_step_random(self, rng, max_x, max_y);
        }

        // Partie fractionnaire : déplacement probabiliste.
        let frac = expected_moves - (steps as f32);
        if frac > 0.0 && rng.gen_bool(frac as f64) {
            Self::one_step_random(self, rng, max_x, max_y);
        }
    }

    fn one_step_random<R: Rng>(&mut self, rng: &mut R, max_x: usize, max_y: usize) {
        // Directions : 0 = haut, 1 = bas, 2 = gauche, 3 = droite
        let dir = rng.gen_range(0..4);

        let mut x = self.pos.x as isize;
        let mut y = self.pos.y as isize;

        match dir {
            0 => y -= 1,
            1 => y += 1,
            2 => x -= 1,
            3 => x += 1,
            _ => {}
        }

        // Clamp entre 0 et max-1
        let max_x_isize = (max_x.saturating_sub(1)) as isize;
        let max_y_isize = (max_y.saturating_sub(1)) as isize;

        if x < 0 {
            x = 0;
        } else if x > max_x_isize {
            x = max_x_isize;
        }

        if y < 0 {
            y = 0;
        } else if y > max_y_isize {
            y = max_y_isize;
        }

        self.pos.x = x as usize;
        self.pos.y = y as usize;
    }
}

impl Combatant for Personnage {
    fn id(&self) -> usize {
        self.id
    }

    fn classe(&self) -> Classe {
        self.classe.clone()
    }

    fn faction(&self) -> Faction {
        self.faction
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