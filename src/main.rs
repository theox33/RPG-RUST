mod entity;
mod game;
mod world;

use game::Game;
use macroquad::prelude::*;

#[macroquad::main("RPG 2D - Fenêtre")]
async fn main() {
    // Chargement des images de textures depuis les fichiers embarqués. Utiliser
    // `include_bytes!` permet de compiler les données dans l'exécutable.
    let map_bytes: &[u8] = include_bytes!("textures/texture_map.png");
    let map_texture = Texture2D::from_file_with_format(map_bytes, None);
    map_texture.set_filter(FilterMode::Nearest);

    let char_bytes: &[u8] = include_bytes!("textures/texture_character.png");
    let char_texture = Texture2D::from_file_with_format(char_bytes, None);
    char_texture.set_filter(FilterMode::Nearest);

    // Créer le jeu avec les textures chargées
    let mut game = Game::new(map_texture, char_texture);
    loop {
        game.frame();
        next_frame().await;
    }
}