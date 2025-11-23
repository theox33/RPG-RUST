use crate::ennemi::Ennemi;
use crate::types::{Combatant, Position};
use crate::world::World;
use rand::{thread_rng, Rng};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::{
    GameTextures, TileType, WorldKind, MAP_HEIGHT, MAP_WIDTH, MAX_ENEMIES, PLAYER_START_X,
    PLAYER_START_Y,
};

pub(super) fn choose_grass_variants(
    textures: &GameTextures,
    tiles: &[Vec<TileType>],
) -> Vec<Vec<usize>> {
    let mut rng = thread_rng();
    (0..MAP_HEIGHT)
        .map(|y| {
            (0..MAP_WIDTH)
                .map(|x| {
                    if tiles[y][x] == TileType::Herbe {
                        rng.gen_range(0..textures.grass_variants.len())
                    } else {
                        0
                    }
                })
                .collect::<Vec<usize>>()
        })
        .collect()
}

pub(super) fn choose_chemin_variants(
    textures: &GameTextures,
    tiles: &[Vec<TileType>],
) -> Vec<Vec<usize>> {
    if textures.chemin_variants.is_empty() {
        return vec![vec![0; MAP_WIDTH]; MAP_HEIGHT];
    }
    let mut rng = thread_rng();
    (0..MAP_HEIGHT)
        .map(|y| {
            (0..MAP_WIDTH)
                .map(|x| {
                    if tiles[y][x] == TileType::Chemin {
                        rng.gen_range(0..textures.chemin_variants.len())
                    } else {
                        0
                    }
                })
                .collect::<Vec<usize>>()
        })
        .collect()
}

pub(super) fn spawn_random_enemies(
    world: &mut World,
    tiles: &[Vec<TileType>],
    max_enemies: usize,
    world_kind: WorldKind,
) {
    let mut rng = thread_rng();
    let mut next_id = 0;
    let is_spirale2 = matches!(world_kind, WorldKind::Spirale2);
    while world.enemies.len() < max_enemies {
        let x = rng.gen_range(0..world.width);
        let y = rng.gen_range(0..world.height);
        if !matches!(tiles[y][x], TileType::Herbe | TileType::Chemin) {
            continue;
        }
        let tile_free = world.players().iter().filter(|p| p.est_vivant()).all(|p| {
            let pos = p.position();
            pos.x != x || pos.y != y
        }) && world.enemies().iter().filter(|e| e.est_vivant()).all(|e| {
            let pos = e.position();
            pos.x != x || pos.y != y
        });
        if tile_free {
            let mut ennemi = Ennemi::nouveau(next_id, Position { x, y });
            if is_spirale2 {
                let stats = ennemi.stats_mut();
                stats.vie = stats.vie.saturating_mul(2);
            }
            world.add_enemy(ennemi);
            next_id += 1;
        }
    }
}

pub(super) fn start_enemy_thread(world: &Arc<Mutex<World>>) {
    let thread_world = Arc::clone(world);
    thread::spawn(move || {
        let tick_ms = 400u64;
        let dt = (tick_ms as f32) / 1000.0;
        loop {
            if let Ok(mut world) = thread_world.lock() {
                world.wander_enemies(dt);
            }
            thread::sleep(Duration::from_millis(tick_ms));
        }
    });
}

pub(super) fn parse_world_file(path: &Path) -> Result<Vec<Vec<TileType>>, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Impossible de lire {:?}: {}", path, e))?;
    let mut rows = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut row = Vec::new();
        for token in trimmed.split(|c: char| c.is_whitespace() || c == ',') {
            if token.is_empty() {
                continue;
            }
            row.push(parse_tile_value(token)?);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    if rows.len() != MAP_HEIGHT {
        return Err(format!(
            "Le fichier {:?} contient {} lignes au lieu de {}",
            path,
            rows.len(),
            MAP_HEIGHT
        ));
    }
    for (y, row) in rows.iter().enumerate() {
        if row.len() != MAP_WIDTH {
            return Err(format!(
                "La ligne {} dans {:?} contient {} colonnes au lieu de {}",
                y,
                path,
                row.len(),
                MAP_WIDTH
            ));
        }
    }
    Ok(rows)
}

fn parse_tile_value(token: &str) -> Result<TileType, String> {
    let value: i32 = token
        .parse()
        .map_err(|_| format!("Valeur de tuile invalide: {}", token))?;
    match value {
        -5 => Ok(TileType::VictoryChest),
        -4 => Ok(TileType::Coffre),
        -3 => Ok(TileType::CollisionInvisible),
        -2 => Ok(TileType::Maison),
        -1 => Ok(TileType::Eau),
        0 => Ok(TileType::Herbe),
        1 | 2 => Ok(TileType::Chemin),
        3 => Ok(TileType::Portal),
        4 => Ok(TileType::SpiralPortal),
        5 => Ok(TileType::SpiralPortal2),
        other => Err(format!("Code tuile inconnu: {}", other)),
    }
}

pub(super) fn detect_house_anchors(tiles: &[Vec<TileType>]) -> Vec<Position> {
    let mut anchors = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if tiles[y][x] != TileType::Maison {
                continue;
            }
            let left_is_house = x > 0 && tiles[y][x - 1] == TileType::Maison;
            let top_is_house = y > 0 && tiles[y - 1][x] == TileType::Maison;
            if !left_is_house && !top_is_house {
                anchors.push(Position { x, y });
            }
        }
    }
    anchors
}

pub(super) fn default_map_tiles() -> Vec<Vec<TileType>> {
    let mut tiles = vec![vec![TileType::Herbe; MAP_WIDTH]; MAP_HEIGHT];
    for y in 0..=PLAYER_START_Y.min(MAP_HEIGHT - 1) {
        tiles[y][PLAYER_START_X] = TileType::Chemin;
    }
    if PLAYER_START_Y < MAP_HEIGHT {
        for x in PLAYER_START_X..MAP_WIDTH {
            tiles[PLAYER_START_Y][x] = TileType::Chemin;
        }
    }
    tiles
}

pub(super) fn load_tiles_for_world(kind: WorldKind) -> Vec<Vec<TileType>> {
    let target = Path::new("worlds").join(kind.filename());
    parse_world_file(&target).unwrap_or_else(|_| load_first_world_in_dir())
}

fn load_first_world_in_dir() -> Vec<Vec<TileType>> {
    let base = Path::new("worlds");
    let _ = fs::create_dir_all(base);
    let mut files = match fs::read_dir(base) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .map(|entry| entry.path())
            .collect::<Vec<_>>(),
        Err(_) => return default_map_tiles(),
    };
    files.sort();
    for path in files {
        if let Ok(tiles) = parse_world_file(&path) {
            return tiles;
        }
    }
    default_map_tiles()
}

pub(super) fn build_walkable_map(tiles: &[Vec<TileType>]) -> Vec<Vec<bool>> {
    tiles
        .iter()
        .map(|row| {
            row.iter()
                .map(|tile| {
                    matches!(
                        tile,
                        TileType::Herbe
                            | TileType::Chemin
                            | TileType::Portal
                            | TileType::SpiralPortal
                            | TileType::SpiralPortal2
                    ) && !matches!(tile, TileType::CollisionInvisible)
                })
                .collect::<Vec<bool>>()
        })
        .collect()
}

pub(super) fn find_tile_position(tiles: &[Vec<TileType>], needle: TileType) -> Option<Position> {
    for (y, row) in tiles.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            if *tile == needle {
                return Some(Position { x, y });
            }
        }
    }
    None
}

pub(super) fn enemy_cap(kind: WorldKind) -> usize {
    match kind {
        WorldKind::Plaine => MAX_ENEMIES,
        WorldKind::Maison => 0,
        WorldKind::Spirale => 5,
        WorldKind::Spirale2 => 5,
    }
}
