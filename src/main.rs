mod entity;
mod game;
mod world;

use game::Game;
use macroquad::prelude::*;

#[macroquad::main("RPG 2D - Fenêtre")]
async fn main() {
    let mut game = Game::new();
    loop {
        game.frame();
        next_frame().await;
    }
}
