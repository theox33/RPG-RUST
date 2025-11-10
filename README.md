RPG 2D tour par tour
====================

Ce projet Rust illustre la mise en place d'un petit RPG en 2D avec **macroquad**. La logique est organisée autour d'une architecture modulaire orientée objet (via traits) :

- `entity.rs` : définit les classes (`Classe`), la structure commune `Personnage` et le trait `Combatant` (points de vie, attaques, vitesse, position, dégâts).
- `world.rs` : gère la grille (12x8), les collisions, le déplacement des entités et le déplacement autonome des ennemis dans un thread dédié (`Arc<Mutex<World>>`).
- `game/mod.rs` : expose la struct `Game`, qui orchestre la boucle principale, les messages, l'état courant (`Exploration` ou `Combat`) ainsi que le rendu.
- `game/combat.rs` : contient `CombatState`, les boutons cliquables (Attaquer, Défendre, Fuir), l'IA basique de l'ennemi et la résolution du tour.

Commandes
---------

- **Déplacements** : Flèches ou ZQSD.
- **Combat** :
	- Clic sur les boutons (ou A/D/F au clavier) pour Attaquer, Défendre ou Fuir.
	- Les messages de combat s'affichent sous la grille.

Lancement
---------

```bash
cargo run
```

Une fenêtre s'ouvre avec la grille, le joueur (bleu) se déplace, les ennemis (rouge) convergent indépendamment. Au contact, le combat tour par tour s'active dans la même fenêtre.

Pour aller plus loin
--------------------

- Ajouter d'autres classes et compétences (sorts, critiques, objets).
- Étendre `Combatant` pour gérer l'expérience, l'équipement ou l'IA avancée.
- Personnaliser le rendu (sprites, animations, interface de combat dédiée).
