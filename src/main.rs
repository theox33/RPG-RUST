mod entity;
mod game;
mod world;

use game::Game;
use macroquad::prelude::*;

#[macroquad::main("RPG 2D - Fenêtre")]
async fn main() {
    // Chargement des images de textures depuis les fichiers embarqués.
    let map_bytes: &[u8] = include_bytes!("textures/texture_map.png");
    let map_texture = Texture2D::from_file_with_format(map_bytes, None);
    map_texture.set_filter(FilterMode::Nearest);

    let char_bytes: &[u8] = include_bytes!("textures/texture_character.png");
    let char_texture = Texture2D::from_file_with_format(char_bytes, None);
    char_texture.set_filter(FilterMode::Nearest);

    // Charger la texture de l'ennemi (slime)
    let enemy_bytes: &[u8] = include_bytes!("textures/ennemi.png");
    let enemy_texture = Texture2D::from_file_with_format(enemy_bytes, None);
    enemy_texture.set_filter(FilterMode::Nearest);

    // Créer le jeu avec toutes les textures chargées
    let mut game = Game::new(map_texture, char_texture, enemy_texture);
    loop {
        game.frame();
        next_frame().await;
    }
}
