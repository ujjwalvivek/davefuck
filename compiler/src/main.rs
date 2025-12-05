use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const COPY_SCRATCH: usize = 73;
const CONTROL_SCRATCH_BASE: usize = 74;
const CONTROL_SCRATCH_LIMIT: usize = 112;
const BF_TOKENS: &[u8] = b"><+-.,[]";
const PLAYER_VERTICAL_OVERLAP_START_SUB_X: usize = 4;
const PLAYER_HEAD_CLEARANCE_SUB_Y: usize = 14;

type PositionMap = BTreeMap<usize, BTreeSet<usize>>;
type LeveledPositionMap = BTreeMap<usize, PositionMap>;
const COLLISION_ROW_MATCH_CELL: usize = 62;

#[derive(Clone, Copy)]
struct Tile {
    x: usize,
    y: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SectionKind {
    Level,
    Transition,
}

struct LevelSection {
    name: String,
    kind: SectionKind,
    level: usize,
    start_x: usize,
    width: usize,
}

struct LeveledTile {
    level: usize,
    tile: Tile,
}

struct CoinSource {
    level: usize,
    tile: Tile,
    kind: char,
}

struct DecorationSource {
    level: usize,
    tile: Tile,
    kind: char,
}

struct SolidMaterialSource {
    tile: Tile,
    kind: char,
}

struct EnemySource {
    level: usize,
    start: Tile,
    min_x: usize,
    max_x: usize,
}

struct LevelSource {
    width: usize,
    height: usize,
    player: Tile,
    keys: Vec<LeveledTile>,
    guns: Vec<LeveledTile>,
    jetpacks: Vec<LeveledTile>,
    flying_enemies: Vec<LeveledTile>,
    coins: Vec<CoinSource>,
    doors: Vec<LeveledTile>,
    enemies: Vec<EnemySource>,
    solids: Vec<Tile>,
    platforms: Vec<Tile>,
    solid_materials: Vec<SolidMaterialSource>,
    decorations: Vec<DecorationSource>,
    solid_set: HashSet<(usize, usize)>,
    platform_set: HashSet<(usize, usize)>,
    min_x: usize,
    max_x: usize,
    ground_y: usize,
    sections: Vec<LevelSection>,
}

#[derive(Clone, Debug)]
struct SourceLine {
    text: String,
    file: PathBuf,
    line: usize,
}

impl SourceLine {
    fn new(text: String, file: PathBuf, line: usize) -> Self {
        Self { text, file, line }
    }

    #[cfg(test)]
    fn test(text: &str, line: usize) -> Self {
        Self::new(text.to_string(), PathBuf::from("<test>"), line)
    }

    fn location(&self) -> String {
        format!("{}:{}", self.file.display(), self.line)
    }

    fn error(&self, message: impl AsRef<str>) -> String {
        format!("{}: {}\n  {}", self.location(), message.as_ref(), self.text)
    }
}

#[derive(Clone)]
struct MacroDef {
    params: Vec<String>,
    body: Vec<SourceLine>,
    defined_at: SourceLine,
}

fn main() {
    if let Err(error) = run_cli() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(());
    }

    match args[0].as_str() {
        "--size" => {
            print!("{}", report_size()?);
            return Ok(());
        }
        "--fmt" => {
            let file = args
                .get(1)
                .ok_or_else(|| "usage: compiler --fmt file.bf".to_string())?;
            let file_path = project_path(file);
            let source =
                fs::read_to_string(&file_path).map_err(|error| format!("read {file}: {error}"))?;
            fs::write(
                &file_path,
                format!("{}\n", format_pretty_brainfuck(&source)?.trim_end()),
            )
            .map_err(|error| format!("write {file}: {error}"))?;
            return Ok(());
        }
        "--fmt-stdin" => {
            let mut source = String::new();
            io::stdin()
                .read_to_string(&mut source)
                .map_err(|error| format!("read stdin: {error}"))?;
            print!("{}", format_pretty_brainfuck(&source)?);
            return Ok(());
        }
        _ => {}
    }

    generate_level_artifacts()?;

    let input = &args[0];
    let bf = assemble_file(input)?;
    if let Some(output_index) = args.iter().position(|arg| arg == "-o") {
        let output = args
            .get(output_index + 1)
            .ok_or_else(|| "missing output path after -o".to_string())?;
        let output_path = project_path(output);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("create {parent:?}: {error}"))?;
        }
        fs::write(output_path, bf).map_err(|error| format!("write {output}: {error}"))?;
    } else {
        print!("{bf}");
    }

    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn project_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root().join(path)
    }
}

fn generate_level_artifacts() -> Result<(), String> {
    let level = read_level_source(project_path("game/level.txt"))?;
    let first_key = level
        .keys
        .first()
        .ok_or_else(|| "generated campaign has no key".to_string())?;
    let first_door = level
        .doors
        .first()
        .ok_or_else(|| "generated campaign has no door".to_string())?;
    let first_enemy = level.enemies.first();
    let level_count = level
        .sections
        .iter()
        .filter(|section| section.kind == SectionKind::Level)
        .count()
        .max(1);
    let player_start_x = local_x_for_level(&level, 1, level.player.x);
    let player_min_x = local_x_for_level(&level, 1, level.min_x);
    let player_max_x = local_x_for_level(&level, 1, level.max_x);
    let first_key_local = local_tile_for_level(&level, first_key.level, first_key.tile);
    let first_door_local = local_tile_for_level(&level, first_door.level, first_door.tile);
    let (down_base, down_overlap, up_base, up_overlap) = build_surface_maps(&level);
    let (
        right_base,
        right_overlap_down,
        left_base,
        left_overlap_down,
        platform_right_base,
        platform_right_overlap_down,
        platform_left_base,
        platform_left_overlap_down,
    ) = build_side_maps(&level);
    let mut collision_macros = Vec::new();
    collision_macros.extend(emit_horizontal_check(
        "check_blocked_right",
        &right_base,
        &right_overlap_down,
        Some((&platform_right_base, &platform_right_overlap_down)),
        level.width - 2,
        &level,
        DoorSide::Right,
    ));
    collision_macros.extend(emit_horizontal_check(
        "check_blocked_left",
        &left_base,
        &left_overlap_down,
        Some((&platform_left_base, &platform_left_overlap_down)),
        1,
        &level,
        DoorSide::Left,
    ));
    collision_macros.extend(emit_vertical_check(
        "check_blocked_up",
        &up_base,
        &up_overlap,
        1,
        &level,
        DoorSide::Up,
    ));
    collision_macros.extend(emit_vertical_check(
        "check_blocked_down",
        &down_base,
        &down_overlap,
        level.height - 2,
        &level,
        DoorSide::Down,
    ));
    collision_macros.extend(emit_projectile_solid_check(&level));
    collision_macros.extend(emit_entity_macros(&level));

    fs::create_dir_all(project_path("game/generated"))
        .map_err(|error| format!("create game/generated: {error}"))?;

    let mut dasm = vec![
        format!("const LEVEL_WIDTH {}", level.width),
        format!("const LEVEL_HEIGHT {}", level.height),
        format!("const LEVEL_COUNT {}", level_count),
        format!("const PLAYER_START_X {}", player_start_x),
        format!("const PLAYER_START_Y {}", level.player.y),
        format!("const PLAYER_MIN_X {}", player_min_x),
        format!("const PLAYER_MAX_X {}", player_max_x),
        "const PLAYER_MIN_Y 1".to_string(),
        format!("const PLAYER_GROUND_Y {}", level.ground_y),
        format!("const PLAYER_HEAD_CLEARANCE_SUB_Y {PLAYER_HEAD_CLEARANCE_SUB_Y}"),
        format!("const KEY_X {}", first_key_local.x),
        format!("const KEY_Y {}", first_key.tile.y),
        format!("const DOOR_X {}", first_door_local.x),
        format!("const DOOR_Y {}", first_door.tile.y),
        format!("const EXIT_X {}", first_door_local.x),
        format!("const EXIT_Y {}", first_door.tile.y),
        format!(
            "const ENEMY_START_X {}",
            first_enemy.map_or(0, |enemy| local_x_for_level(
                &level,
                enemy.level,
                enemy.start.x
            ))
        ),
        format!(
            "const ENEMY_START_Y {}",
            first_enemy.map_or(0, |enemy| enemy.start.y)
        ),
        format!(
            "const ENEMY_MIN_X {}",
            first_enemy.map_or(0, |enemy| local_x_for_level(
                &level,
                enemy.level,
                enemy.min_x
            ))
        ),
        format!(
            "const ENEMY_MAX_X {}",
            first_enemy.map_or(0, |enemy| local_x_for_level(
                &level,
                enemy.level,
                enemy.max_x
            ))
        ),
        "const ENEMY_START_DIR 1".to_string(),
        "const ENEMY_TICK_DELAY 18".to_string(),
        format!("const COLLISION_ROW_MATCH {COLLISION_ROW_MATCH_CELL}"),
    ];
    for key in &level.keys {
        dasm.push(format!(
            "const KEY_{}_X {}",
            key.level,
            local_x_for_level(&level, key.level, key.tile.x)
        ));
        dasm.push(format!("const KEY_{}_Y {}", key.level, key.tile.y));
    }
    for door in &level.doors {
        dasm.push(format!(
            "const DOOR_{}_X {}",
            door.level,
            local_x_for_level(&level, door.level, door.tile.x)
        ));
        dasm.push(format!("const DOOR_{}_Y {}", door.level, door.tile.y));
    }
    for enemy in &level.enemies {
        dasm.push(format!(
            "const ENEMY_{}_START_X {}",
            enemy.level,
            local_x_for_level(&level, enemy.level, enemy.start.x)
        ));
        dasm.push(format!(
            "const ENEMY_{}_START_Y {}",
            enemy.level, enemy.start.y
        ));
        dasm.push(format!(
            "const ENEMY_{}_MIN_X {}",
            enemy.level,
            local_x_for_level(&level, enemy.level, enemy.min_x)
        ));
        dasm.push(format!(
            "const ENEMY_{}_MAX_X {}",
            enemy.level,
            local_x_for_level(&level, enemy.level, enemy.max_x)
        ));
    }
    let level_count = level
        .sections
        .iter()
        .filter(|section| section.kind == SectionKind::Level)
        .count()
        .max(1);
    for lvl in 1..=level_count {
        let level_enemies: Vec<_> = level
            .flying_enemies
            .iter()
            .filter(|e| e.level == lvl)
            .collect();
        for (idx, enemy) in level_enemies.iter().enumerate() {
            dasm.push(format!(
                "const FLYING_ENEMY_{}_{}_START_X {}",
                lvl,
                idx,
                local_x_for_level(&level, lvl, enemy.tile.x)
            ));
            dasm.push(format!(
                "const FLYING_ENEMY_{}_{}_START_Y {}",
                lvl,
                idx,
                enemy.tile.y.saturating_sub(1)
            ));
        }
    }
    let coin_count = level.coins.len();
    for index in 0..coin_count {
        dasm.push(format!("const COIN_{index}_COLLECTED {}", 128 + index));
    }
    dasm.extend([format!("const COIN_COUNT {coin_count}"), String::new()]);
    for index in 0..level.jetpacks.len() {
        dasm.push(format!(
            "const JETPACK_{index}_COLLECTED {}",
            128 + coin_count + index
        ));
    }
    dasm.extend([
        format!("const JETPACK_COUNT {}", level.jetpacks.len()),
        String::new(),
    ]);
    dasm.extend(collision_macros);
    dasm.push(String::new());
    fs::write(project_path("game/generated/level.dasm"), dasm.join("\n"))
        .map_err(|error| format!("write game/generated/level.dasm: {error}"))?;

    let solids_json = level
        .solids
        .iter()
        .map(|tile| format!("{{\"x\":{},\"y\":{}}}", tile.x, tile.y))
        .collect::<Vec<_>>()
        .join(",");
    let platforms_json = level
        .platforms
        .iter()
        .map(|tile| format!("{{\"x\":{},\"y\":{}}}", tile.x, tile.y))
        .collect::<Vec<_>>()
        .join(",");
    let solid_materials_json = level
        .solid_materials
        .iter()
        .map(|material| {
            format!(
                "{{\"x\":{},\"y\":{},\"kind\":\"{}\"}}",
                material.tile.x, material.tile.y, material.kind
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let coins_json = level
        .coins
        .iter()
        .map(|coin| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{},\"kind\":\"{}\"}}",
                coin.level, coin.tile.x, coin.tile.y, coin.kind
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let guns_json = level
        .guns
        .iter()
        .map(|gun| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{}}}",
                gun.level, gun.tile.x, gun.tile.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let jetpacks_json = level
        .jetpacks
        .iter()
        .map(|jetpack| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{}}}",
                jetpack.level, jetpack.tile.x, jetpack.tile.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let flying_enemies_json = level
        .flying_enemies
        .iter()
        .map(|f| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{}}}",
                f.level, f.tile.x, f.tile.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let decorations_json = level
        .decorations
        .iter()
        .map(|decoration| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{},\"kind\":\"{}\"}}",
                decoration.level, decoration.tile.x, decoration.tile.y, decoration.kind
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let keys_json = level
        .keys
        .iter()
        .map(|key| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{}}}",
                key.level, key.tile.x, key.tile.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let doors_json = level
        .doors
        .iter()
        .map(|door| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{}}}",
                door.level, door.tile.x, door.tile.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let enemies_json = level
        .enemies
        .iter()
        .map(|enemy| {
            format!(
                "{{\"level\":{},\"x\":{},\"y\":{},\"minX\":{},\"maxX\":{},\"tickDelay\":18}}",
                enemy.level, enemy.start.x, enemy.start.y, enemy.min_x, enemy.max_x
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let sections_json = level
        .sections
        .iter()
        .map(|section| {
            let kind = match section.kind {
                SectionKind::Level => "level",
                SectionKind::Transition => "transition",
            };
            format!(
                "{{\"name\":\"{}\",\"kind\":\"{}\",\"level\":{},\"startX\":{},\"width\":{}}}",
                section.name, kind, section.level, section.start_x, section.width
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    fs::write(
        project_path("game/generated/level.js"),
        [
            "export const level = Object.freeze({".to_string(),
            format!("  width: {},", level.width),
            format!("  height: {},", level.height),
            format!("  levelCount: {},", level_count),
            format!(
                "  playerStart: Object.freeze({{ x: {}, y: {} }}),",
                level.player.x, level.player.y
            ),
            format!("  playerGroundY: {},", level.ground_y),
            format!("  sections: Object.freeze([{sections_json}]),"),
            format!("  coins: Object.freeze([{coins_json}]),"),
            format!("  guns: Object.freeze([{guns_json}]),"),
            format!("  jetpacks: Object.freeze([{jetpacks_json}]),"),
            format!("  flyingEnemies: Object.freeze([{flying_enemies_json}]),"),
            format!("  decorations: Object.freeze([{decorations_json}]),"),
            format!("  keys: Object.freeze([{keys_json}]),"),
            format!("  doors: Object.freeze([{doors_json}]),"),
            format!("  enemies: Object.freeze([{enemies_json}]),"),
            format!(
                "  key: Object.freeze({{ x: {}, y: {} }}),",
                first_key.tile.x, first_key.tile.y
            ),
            format!(
                "  door: Object.freeze({{ x: {}, y: {} }}),",
                first_door.tile.x, first_door.tile.y
            ),
            format!(
                "  exit: Object.freeze({{ x: {}, y: {} }}),",
                first_door.tile.x, first_door.tile.y
            ),
            first_enemy.map_or_else(
                || "  enemy: null,".to_string(),
                |enemy| {
                    format!(
                        "  enemy: Object.freeze({{\n    x: {},\n    y: {},\n    minX: {},\n    maxX: {},\n    tickDelay: 18\n  }}),",
                        enemy.start.x, enemy.start.y, enemy.min_x, enemy.max_x
                    )
                },
            ),
            format!("  platforms: Object.freeze([{platforms_json}]),"),
            format!("  solidMaterials: Object.freeze([{solid_materials_json}]),"),
            format!("  solids: Object.freeze([{solids_json}])"),
            "});".to_string(),
            String::new(),
        ]
        .join("\n"),
    )
    .map_err(|error| format!("write game/generated/level.js: {error}"))?;

    let mut level_rs = vec![
        format!("pub const LEVEL_WIDTH: u8 = {};", level.width),
        format!("pub const LEVEL_HEIGHT: u8 = {};", level.height),
        format!("pub const LEVEL_COUNT: u8 = {};", level_count),
        format!("pub const PLAYER_START_X: u8 = {};", level.player.x),
        format!("pub const PLAYER_START_Y: u8 = {};", level.player.y),
        format!("pub const PLAYER_GROUND_Y: u8 = {};", level.ground_y),
        format!("pub const KEY_X: u8 = {};", first_key.tile.x),
        format!("pub const KEY_Y: u8 = {};", first_key.tile.y),
        "pub const KEYS: &[(u8, u8, u8)] = &[".to_string(),
    ];
    level_rs.extend(
        level
            .keys
            .iter()
            .map(|key| format!("    ({}, {}, {}),", key.level, key.tile.x, key.tile.y)),
    );
    level_rs.extend([
        "];".to_string(),
        "pub const COINS: &[(u8, u8)] = &[".to_string(),
    ]);
    level_rs.extend(
        level
            .coins
            .iter()
            .map(|coin| format!("    ({}, {}),", coin.tile.x, coin.tile.y)),
    );
    level_rs.extend([
        "];".to_string(),
        format!("pub const DOOR_X: u8 = {};", first_door.tile.x),
        format!("pub const DOOR_Y: u8 = {};", first_door.tile.y),
        format!("pub const EXIT_X: u8 = {};", first_door.tile.x),
        format!("pub const EXIT_Y: u8 = {};", first_door.tile.y),
        "pub const DOORS: &[(u8, u8, u8)] = &[".to_string(),
    ]);
    level_rs.extend(
        level
            .doors
            .iter()
            .map(|door| format!("    ({}, {}, {}),", door.level, door.tile.x, door.tile.y)),
    );
    level_rs.extend([
        "];".to_string(),
        format!(
            "pub const ENEMY_START_X: u8 = {};",
            first_enemy.map_or(0, |enemy| enemy.start.x)
        ),
        format!(
            "pub const ENEMY_START_Y: u8 = {};",
            first_enemy.map_or(0, |enemy| enemy.start.y)
        ),
        format!(
            "pub const ENEMY_MIN_X: u8 = {};",
            first_enemy.map_or(0, |enemy| enemy.min_x)
        ),
        format!(
            "pub const ENEMY_MAX_X: u8 = {};",
            first_enemy.map_or(0, |enemy| enemy.max_x)
        ),
        "pub const ENEMIES: &[(u8, u8, u8, u8, u8)] = &[".to_string(),
    ]);
    level_rs.extend(level.enemies.iter().map(|enemy| {
        format!(
            "    ({}, {}, {}, {}, {}),",
            enemy.level, enemy.start.x, enemy.start.y, enemy.min_x, enemy.max_x
        )
    }));
    level_rs.extend([
        "];".to_string(),
        "pub const LEVEL_SOLIDS: &[(u8, u8)] = &[".to_string(),
    ]);
    level_rs.extend(
        level
            .solids
            .iter()
            .map(|tile| format!("    ({}, {}),", tile.x, tile.y)),
    );
    level_rs.push("];".to_string());
    level_rs.push(String::new());
    fs::write(project_path("game/generated/level.rs"), level_rs.join("\n"))
        .map_err(|error| format!("write game/generated/level.rs: {error}"))?;

    fs::write(
        project_path("game/generated/level.meta"),
        [
            format!("LEVEL_WIDTH={}", level.width),
            format!("LEVEL_HEIGHT={}", level.height),
            format!("LEVEL_COUNT={}", level_count),
            format!("PLAYER_START_X={}", level.player.x),
            format!("PLAYER_START_Y={}", level.player.y),
            format!("PLAYER_GROUND_Y={}", level.ground_y),
            format!(
                "COINS={}",
                level
                    .coins
                    .iter()
                    .map(|coin| format!("{},{}", coin.tile.x, coin.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "GUNS={}",
                level
                    .guns
                    .iter()
                    .map(|gun| format!("{},{},{}", gun.level, gun.tile.x, gun.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "JETPACKS={}",
                level
                    .jetpacks
                    .iter()
                    .map(|j| format!("{},{},{}", j.level, j.tile.x, j.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "FLYING_ENEMIES={}",
                level
                    .flying_enemies
                    .iter()
                    .map(|f| format!("{},{},{}", f.level, f.tile.x, f.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!("KEY_X={}", first_key.tile.x),
            format!("KEY_Y={}", first_key.tile.y),
            format!(
                "KEYS={}",
                level
                    .keys
                    .iter()
                    .map(|key| format!("{},{},{}", key.level, key.tile.x, key.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!("DOOR_X={}", first_door.tile.x),
            format!("DOOR_Y={}", first_door.tile.y),
            format!("EXIT_X={}", first_door.tile.x),
            format!("EXIT_Y={}", first_door.tile.y),
            format!(
                "DOORS={}",
                level
                    .doors
                    .iter()
                    .map(|door| format!("{},{},{}", door.level, door.tile.x, door.tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "ENEMY_START_X={}",
                first_enemy.map_or(0, |enemy| enemy.start.x)
            ),
            format!(
                "ENEMY_START_Y={}",
                first_enemy.map_or(0, |enemy| enemy.start.y)
            ),
            format!("ENEMY_MIN_X={}", first_enemy.map_or(0, |enemy| enemy.min_x)),
            format!("ENEMY_MAX_X={}", first_enemy.map_or(0, |enemy| enemy.max_x)),
            format!(
                "ENEMIES={}",
                level
                    .enemies
                    .iter()
                    .map(|enemy| format!(
                        "{},{},{},{},{}",
                        enemy.level, enemy.start.x, enemy.start.y, enemy.min_x, enemy.max_x
                    ))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "SECTIONS={}",
                level
                    .sections
                    .iter()
                    .map(|section| {
                        let kind = match section.kind {
                            SectionKind::Level => "level",
                            SectionKind::Transition => "transition",
                        };
                        format!(
                            "{},{},{},{}",
                            kind, section.level, section.start_x, section.width
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "HAZARDS={}",
                level
                    .decorations
                    .iter()
                    .filter(|decoration| matches!(decoration.kind, 'F' | 'W' | 'V'))
                    .map(|decoration| format!(
                        "{},{},{}",
                        decoration.level, decoration.tile.x, decoration.tile.y
                    ))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            format!(
                "LEVEL_SOLIDS={}",
                level
                    .solids
                    .iter()
                    .map(|tile| format!("{},{}", tile.x, tile.y))
                    .collect::<Vec<_>>()
                    .join(";")
            ),
            String::new(),
        ]
        .join("\n"),
    )
    .map_err(|error| format!("write game/generated/level.meta: {error}"))?;

    Ok(())
}

fn read_level_source(filename: impl AsRef<Path>) -> Result<LevelSource, String> {
    let filename = filename.as_ref();
    let levels_dir = project_path("game/levels");
    if levels_dir.is_dir() {
        let mut files = fs::read_dir(&levels_dir)
            .map_err(|error| format!("read {}: {error}", levels_dir.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("txt"))
            .collect::<Vec<_>>();
        files.sort();
        if !files.is_empty() {
            return read_campaign_source(&files);
        }
    }

    let text = fs::read_to_string(filename)
        .map_err(|error| format!("read {}: {error}", filename.display()))?;
    let rows = parse_rows(&text, filename)?;
    build_level_source(
        rows,
        vec![LevelSection {
            name: "01".to_string(),
            kind: SectionKind::Level,
            level: 1,
            start_x: 0,
            width: 0,
        }],
    )
}

fn read_campaign_source(files: &[PathBuf]) -> Result<LevelSource, String> {
    let mut stitched_rows: Vec<Vec<char>> = Vec::new();
    let mut sections = Vec::new();
    let mut cursor_x = 0usize;
    let mut expected_height = None;

    for (index, file) in files.iter().enumerate() {
        let text = fs::read_to_string(file)
            .map_err(|error| format!("read {}: {error}", file.display()))?;
        let rows = parse_rows(&text, file)?;
        let height = rows.len();
        if let Some(expected) = expected_height {
            if height != expected {
                return Err(format!(
                    "{} has height {height}; expected {expected}",
                    file.display()
                ));
            }
        } else {
            expected_height = Some(height);
            stitched_rows = vec![Vec::new(); height];
        }

        if index > 0 {
            let transition = transition_rows(height, 12);
            sections.push(LevelSection {
                name: format!("transition-{}", index),
                kind: SectionKind::Transition,
                level: index + 1,
                start_x: cursor_x,
                width: transition[0].len(),
            });
            append_rows(&mut stitched_rows, &transition);
            cursor_x += transition[0].len();
        }

        let section_width = rows[0].len();
        sections.push(LevelSection {
            name: file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("level")
                .to_string(),
            kind: SectionKind::Level,
            level: index + 1,
            start_x: cursor_x,
            width: section_width,
        });
        append_rows(&mut stitched_rows, &rows);
        cursor_x += section_width;
    }

    build_level_source(stitched_rows, sections)
}

fn parse_rows(text: &str, filename: &Path) -> Result<Vec<Vec<char>>, String> {
    let rows: Vec<Vec<char>> = text
        .trim_end()
        .lines()
        .map(|line| line.chars().collect())
        .collect();
    let height = rows.len();
    let width = rows.first().map_or(0, Vec::len);
    if width == 0 || height == 0 {
        return Err(format!("{} is empty", filename.display()));
    }
    if rows.iter().any(|row| row.len() != width) {
        return Err(format!(
            "{} rows must all be the same width",
            filename.display()
        ));
    }
    Ok(rows)
}

fn append_rows(target: &mut [Vec<char>], source: &[Vec<char>]) {
    for (target_row, source_row) in target.iter_mut().zip(source) {
        target_row.extend(source_row);
    }
}

fn transition_rows(height: usize, width: usize) -> Vec<Vec<char>> {
    let mut rows = vec![vec!['.'; width]; height];
    for x in 0..width {
        rows[0][x] = '#';
        rows[height - 1][x] = '#';
    }
    for row in rows.iter_mut().take(height - 1).skip(1) {
        row[0] = '#';
        row[width - 1] = '#';
    }
    if height > 4 {
        let message_y = height / 2;
        for x in 2..width.saturating_sub(2) {
            rows[message_y][x] = '.';
        }
    }
    rows
}

fn build_level_source(
    rows: Vec<Vec<char>>,
    mut sections: Vec<LevelSection>,
) -> Result<LevelSource, String> {
    let height = rows.len();
    let width = rows.first().map_or(0, Vec::len);
    for section in &mut sections {
        if section.width == 0 {
            section.width = width;
        }
    }

    let mut player = None;
    let mut keys = Vec::new();
    let mut guns = Vec::new();
    let mut jetpacks = Vec::new();
    let mut flying_enemies = Vec::new();
    let mut coins = Vec::new();
    let mut doors = Vec::new();
    let mut enemies = Vec::new();
    let mut raw_enemies = Vec::new();
    let mut solids = Vec::new();
    let mut platforms = Vec::new();
    let mut solid_materials = Vec::new();
    let mut decorations = Vec::new();
    let mut solid_set = HashSet::new();
    let mut platform_set = HashSet::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, tile) in row.iter().enumerate() {
            let level_index = section_level_for_x(&sections, x);
            let tile_kind = tile.to_ascii_uppercase();
            if matches!(tile_kind, '#' | '=' | '@' | 'U') {
                solids.push(Tile { x, y });
                solid_set.insert((x, y));
            }
            if matches!(tile_kind, '@' | 'U') {
                solid_materials.push(SolidMaterialSource {
                    tile: Tile { x, y },
                    kind: tile_kind,
                });
            }
            if tile_kind == '=' {
                platforms.push(Tile { x, y });
                platform_set.insert((x, y));
            }
            if *tile == 'P' {
                if player.is_some() {
                    return Err("Level must contain exactly one player start tile P".to_string());
                }
                player = Some(Tile { x, y });
            }
            if *tile == 'K' {
                keys.push(LeveledTile {
                    level: level_index,
                    tile: Tile { x, y },
                });
            }
            if tile_kind == 'G' {
                guns.push(LeveledTile {
                    level: level_index,
                    tile: Tile { x, y },
                });
            }
            if tile_kind == 'J' {
                jetpacks.push(LeveledTile {
                    level: level_index,
                    tile: Tile { x, y },
                });
            }
            if tile_kind == 'Z' {
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dy == 0 && dx == 0 {
                            continue;
                        }
                        let check_x = x as isize + dx;
                        let check_y = y as isize + dy;
                        if check_x < 0
                            || check_y < 0
                            || check_x >= width as isize
                            || check_y >= height as isize
                        {
                            return Err(format!(
                                "Flying enemy at ({x}, {y}) is too close to level borders"
                            ));
                        }
                        let neighbor = rows[check_y as usize][check_x as usize];
                        if neighbor != '.' {
                            return Err(format!(
                                "Flying enemy at ({x}, {y}) must be surrounded by '.' in a 3x3 grid (found '{neighbor}' at ({check_x}, {check_y}))"
                            ));
                        }
                    }
                }
                flying_enemies.push(LeveledTile {
                    level: level_index,
                    tile: Tile { x, y },
                });
            }
            if matches!(tile_kind, 'C' | 'R' | 'B') {
                coins.push(CoinSource {
                    level: level_index,
                    tile: Tile { x, y },
                    kind: tile_kind,
                });
            }
            if *tile == 'D' {
                doors.push(LeveledTile {
                    level: level_index,
                    tile: Tile { x, y },
                });
            }
            if *tile == 'M' {
                raw_enemies.push((level_index, Tile { x, y }));
            }
            if matches!(tile_kind, 'F' | 'V' | 'W') {
                decorations.push(DecorationSource {
                    level: level_index,
                    tile: Tile { x, y },
                    kind: tile_kind,
                });
            }
        }
    }
    let player = player.ok_or_else(|| "Level must contain a player start tile P".to_string())?;
    if keys.is_empty() {
        return Err("Campaign must contain at least one key tile K".to_string());
    }
    if doors.is_empty() {
        return Err("Campaign must contain at least one door tile D".to_string());
    }
    let mut min_x = player.x;
    while min_x > 0
        && !is_solid_at(
            &solid_set,
            width,
            height,
            min_x as isize - 1,
            player.y as isize,
        )
    {
        min_x -= 1;
    }

    let mut max_x = player.x;
    while max_x < width - 1
        && !is_solid_at(
            &solid_set,
            width,
            height,
            max_x as isize + 1,
            player.y as isize,
        )
    {
        max_x += 1;
    }

    let mut ground_y = player.y;
    while ground_y < height - 1
        && !is_solid_at(
            &solid_set,
            width,
            height,
            player.x as isize,
            ground_y as isize + 1,
        )
    {
        ground_y += 1;
    }

    for (level, enemy) in raw_enemies {
        let mut enemy_min_x = enemy.x;
        while enemy_min_x > 1
            && !is_solid_at(
                &solid_set,
                width,
                height,
                enemy_min_x as isize - 1,
                enemy.y as isize,
            )
            && is_solid_at(
                &solid_set,
                width,
                height,
                enemy_min_x as isize - 1,
                enemy.y as isize + 1,
            )
        {
            enemy_min_x -= 1;
        }

        let mut enemy_max_x = enemy.x;
        while enemy_max_x < width - 2
            && !is_solid_at(
                &solid_set,
                width,
                height,
                enemy_max_x as isize + 1,
                enemy.y as isize,
            )
            && is_solid_at(
                &solid_set,
                width,
                height,
                enemy_max_x as isize + 1,
                enemy.y as isize + 1,
            )
        {
            enemy_max_x += 1;
        }
        if enemy_min_x == enemy_max_x {
            return Err("Enemy tile M must have at least two supported patrol tiles".to_string());
        }
        enemies.push(EnemySource {
            level,
            start: enemy,
            min_x: enemy_min_x,
            max_x: enemy_max_x,
        });
    }
    keys.sort_by_key(|key| (key.level, key.tile.x, key.tile.y));
    coins.sort_by_key(|coin| (coin.level, coin.tile.x, coin.tile.y, coin.kind));
    doors.sort_by_key(|door| (door.level, door.tile.x, door.tile.y));
    enemies.sort_by_key(|enemy| (enemy.level, enemy.start.x, enemy.start.y));
    flying_enemies.sort_by_key(|enemy| (enemy.level, enemy.tile.x, enemy.tile.y));

    Ok(LevelSource {
        width,
        height,
        player,
        keys,
        guns,
        jetpacks,
        flying_enemies,
        coins,
        doors,
        enemies,
        solids,
        platforms,
        solid_materials,
        decorations,
        solid_set,
        platform_set,
        min_x,
        max_x,
        ground_y,
        sections,
    })
}

fn section_level_for_x(sections: &[LevelSection], x: usize) -> usize {
    sections
        .iter()
        .filter(|section| x >= section.start_x && x < section.start_x + section.width)
        .map(|section| section.level)
        .next()
        .unwrap_or(1)
}

fn level_section(level: &LevelSource, level_index: usize) -> Option<&LevelSection> {
    level
        .sections
        .iter()
        .find(|section| section.kind == SectionKind::Level && section.level == level_index)
}

fn level_section_for_x(level: &LevelSource, x: usize) -> Option<&LevelSection> {
    level.sections.iter().find(|section| {
        section.kind == SectionKind::Level
            && x >= section.start_x
            && x < section.start_x + section.width
    })
}

fn local_x_for_level(level: &LevelSource, level_index: usize, x: usize) -> usize {
    level_section(level, level_index).map_or(x, |section| x.saturating_sub(section.start_x))
}

fn local_tile_for_level(level: &LevelSource, level_index: usize, tile: Tile) -> Tile {
    Tile {
        x: local_x_for_level(level, level_index, tile.x),
        y: tile.y,
    }
}

fn is_solid_at(
    solid_set: &HashSet<(usize, usize)>,
    width: usize,
    height: usize,
    x: isize,
    y: isize,
) -> bool {
    if x < 0 || y < 0 || x as usize >= width || y as usize >= height {
        return true;
    }
    solid_set.contains(&(x as usize, y as usize))
}

fn is_solid(level: &LevelSource, x: isize, y: isize) -> bool {
    if x < 0 || y < 0 || x as usize >= level.width || y as usize >= level.height {
        return true;
    }
    level.solid_set.contains(&(x as usize, y as usize))
}

fn is_full_solid(level: &LevelSource, x: isize, y: isize) -> bool {
    if x < 0 || y < 0 || x as usize >= level.width || y as usize >= level.height {
        return true;
    }
    let position = (x as usize, y as usize);
    level.solid_set.contains(&position) && !level.platform_set.contains(&position)
}

fn is_platform(level: &LevelSource, x: isize, y: isize) -> bool {
    if x < 0 || y < 0 || x as usize >= level.width || y as usize >= level.height {
        return false;
    }
    level.platform_set.contains(&(x as usize, y as usize))
}

fn add_position(map: &mut PositionMap, level: &LevelSource, y: isize, x: isize) {
    if x < 1 || y < 1 || x > level.width as isize - 2 || y > level.height as isize - 2 {
        return;
    }
    map.entry(y as usize).or_default().insert(x as usize);
}

fn ranges_for_row(
    level: &LevelSource,
    y: usize,
    solid: fn(&LevelSource, isize, isize) -> bool,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;
    for x in 1..level.width - 1 {
        if solid(level, x as isize, y as isize) {
            if start.is_none() {
                start = Some(x);
            }
        } else if let Some(range_start) = start.take() {
            ranges.push((range_start, x - 1));
        }
    }
    if let Some(range_start) = start {
        ranges.push((range_start, level.width - 2));
    }
    ranges
}

fn build_surface_maps(level: &LevelSource) -> (PositionMap, PositionMap, PositionMap, PositionMap) {
    let mut down_base = PositionMap::new();
    let mut down_overlap = PositionMap::new();
    let mut up_base = PositionMap::new();
    let mut up_overlap = PositionMap::new();

    for y in 1..level.height - 1 {
        for (start, end) in ranges_for_row(level, y, is_full_solid) {
            let top_y = y as isize - 1;
            let bottom_y = y as isize + 1;
            for x in start..=end {
                add_position(&mut down_base, level, top_y, x as isize);
                add_position(&mut up_base, level, bottom_y, x as isize);
            }
            add_position(&mut down_overlap, level, top_y, start as isize - 1);
            add_position(&mut up_overlap, level, bottom_y, start as isize - 1);
        }
        for (start, end) in ranges_for_row(level, y, is_platform) {
            let top_y = y as isize - 1;
            for x in start..=end {
                add_position(&mut down_base, level, top_y, x as isize);
            }
            add_position(&mut down_overlap, level, top_y, start as isize - 1);
        }
    }

    (down_base, down_overlap, up_base, up_overlap)
}

fn build_side_maps(
    level: &LevelSource,
) -> (
    PositionMap,
    PositionMap,
    PositionMap,
    PositionMap,
    PositionMap,
    PositionMap,
    PositionMap,
    PositionMap,
) {
    let mut right_base = PositionMap::new();
    let mut right_overlap_down = PositionMap::new();
    let mut left_base = PositionMap::new();
    let mut left_overlap_down = PositionMap::new();
    let mut platform_right_base = PositionMap::new();
    let mut platform_right_overlap_down = PositionMap::new();
    let mut platform_left_base = PositionMap::new();
    let mut platform_left_overlap_down = PositionMap::new();

    for y in 1..level.height - 1 {
        for x in 1..level.width - 1 {
            if is_full_solid(level, x as isize, y as isize) {
                if !is_solid(level, x as isize - 1, y as isize) {
                    add_position(&mut right_base, level, y as isize, x as isize - 1);
                    add_position(
                        &mut right_overlap_down,
                        level,
                        y as isize - 1,
                        x as isize - 1,
                    );
                }
                if !is_solid(level, x as isize + 1, y as isize) {
                    add_position(&mut left_base, level, y as isize, x as isize + 1);
                    add_position(
                        &mut left_overlap_down,
                        level,
                        y as isize - 1,
                        x as isize + 1,
                    );
                }
            }
            if is_platform(level, x as isize, y as isize)
                && !is_solid(level, x as isize - 1, y as isize)
            {
                add_position(&mut platform_right_base, level, y as isize, x as isize - 1);
                add_position(
                    &mut platform_right_overlap_down,
                    level,
                    y as isize - 1,
                    x as isize - 1,
                );
            }
            if is_platform(level, x as isize, y as isize)
                && !is_solid(level, x as isize + 1, y as isize)
            {
                add_position(&mut platform_left_base, level, y as isize, x as isize + 1);
                add_position(
                    &mut platform_left_overlap_down,
                    level,
                    y as isize - 1,
                    x as isize + 1,
                );
            }
        }
    }

    (
        right_base,
        right_overlap_down,
        left_base,
        left_overlap_down,
        platform_right_base,
        platform_right_overlap_down,
        platform_left_base,
        platform_left_overlap_down,
    )
}

fn leveled_position_map(level: &LevelSource, map: &PositionMap) -> LeveledPositionMap {
    let mut leveled = LeveledPositionMap::new();
    for (y, xs) in map {
        for x in xs {
            let Some(section) = level_section_for_x(level, *x) else {
                continue;
            };
            leveled
                .entry(section.level)
                .or_default()
                .entry(*y)
                .or_default()
                .insert(x.saturating_sub(section.start_x));
        }
    }
    leveled
}

fn emit_position_map_body(
    lines: &mut Vec<String>,
    map: &PositionMap,
    indent: &str,
    section_width: Option<usize>,
) {
    for (y, xs) in map.iter().rev() {
        let gap_count = section_width.map_or(usize::MAX, |w| {
            let inner = if w >= 2 { w - 2 } else { 0 };
            inner.saturating_sub(xs.len())
        });
        if xs.len() > gap_count {
            lines.push(format!("{indent}ifeq PLAYER_Y {y}"));
            lines.push(format!("{indent}  set COLLISION_ROW_MATCH 1"));
            if let Some(w) = section_width {
                for x in 1..w.saturating_sub(1) {
                    if !xs.contains(&x) {
                        lines.push(format!("{indent}  ifeq PLAYER_X {x}"));
                        lines.push(format!("{indent}    clear COLLISION_ROW_MATCH"));
                        lines.push(format!("{indent}  end"));
                    }
                }
            }
            lines.push(format!("{indent}  ifnz COLLISION_ROW_MATCH"));
            lines.push(format!("{indent}    set COLLISION_BLOCKED 1"));
            lines.push(format!("{indent}  end"));
            lines.push(format!("{indent}  clear COLLISION_ROW_MATCH"));
            lines.push(format!("{indent}end"));
        } else {
            lines.push(format!("{indent}ifeq PLAYER_Y {y}"));
            for x in xs.iter().rev() {
                lines.push(format!("{indent}  ifeq PLAYER_X {x}"));
                lines.push(format!("{indent}    set COLLISION_BLOCKED 1"));
                lines.push(format!("{indent}  end"));
            }
            lines.push(format!("{indent}end"));
        }
    }
}

fn section_width_for_level(level: &LevelSource, level_index: usize) -> Option<usize> {
    level_section(level, level_index).map(|s| s.width)
}

fn emit_position_map(
    lines: &mut Vec<String>,
    level: &LevelSource,
    map: &PositionMap,
    indent: &str,
) {
    for (level_index, map) in leveled_position_map(level, map).iter().rev() {
        let sw = section_width_for_level(level, *level_index);
        lines.push(format!("{indent}ifeq CURRENT_LEVEL {level_index}"));
        emit_position_map_body(lines, map, &format!("{indent}  "), sw);
        lines.push(format!("{indent}end"));
    }
}

fn emit_vertical_overlap_map(
    lines: &mut Vec<String>,
    level: &LevelSource,
    map: &PositionMap,
    indent: &str,
) {
    lines.push(format!("{indent}ifnz PLAYER_SUB_X"));
    for sub_x in 1..PLAYER_VERTICAL_OVERLAP_START_SUB_X {
        lines.push(format!("{indent}  ifneq PLAYER_SUB_X {sub_x}"));
    }
    let gated_indent = format!(
        "{indent}{}",
        "  ".repeat(PLAYER_VERTICAL_OVERLAP_START_SUB_X)
    );
    emit_position_map(lines, level, map, &gated_indent);
    for _ in 1..PLAYER_VERTICAL_OVERLAP_START_SUB_X {
        lines.push(format!("{indent}  end"));
    }
    lines.push(format!("{indent}end"));
}

fn emit_platform_side_map(
    lines: &mut Vec<String>,
    level: &LevelSource,
    map: &PositionMap,
    indent: &str,
) {
    lines.push(format!("{indent}ifneq PLAYER_SUB_Y 12"));
    lines.push(format!("{indent}  ifneq PLAYER_SUB_Y 14"));
    emit_position_map(lines, level, map, &format!("{indent}    "));
    lines.push(format!("{indent}  end"));
    lines.push(format!("{indent}end"));
}

#[derive(Clone, Copy)]
enum DoorSide {
    Right,
    Left,
    Up,
    Down,
}

fn emit_door_position(
    lines: &mut Vec<String>,
    level: &LevelSource,
    indent: &str,
    door: &LeveledTile,
    y: usize,
    x: usize,
) {
    let x = local_x_for_level(level, door.level, x);
    lines.push(format!("{indent}ifeq CURRENT_LEVEL {}", door.level));
    lines.push(format!("{indent}  ifz DOOR_OPEN"));
    lines.push(format!("{indent}    ifeq PLAYER_Y {y}"));
    lines.push(format!("{indent}      ifeq PLAYER_X {x}"));
    lines.push(format!("{indent}        set COLLISION_BLOCKED 1"));
    lines.push(format!("{indent}      end"));
    lines.push(format!("{indent}    end"));
    lines.push(format!("{indent}  end"));
    lines.push(format!("{indent}end"));
}

fn emit_door_horizontal_check(
    lines: &mut Vec<String>,
    level: &LevelSource,
    side: DoorSide,
    indent: &str,
) {
    for door in &level.doors {
        match side {
            DoorSide::Right if door.tile.x > 0 => {
                emit_door_position(lines, level, indent, door, door.tile.y, door.tile.x - 1);
            }
            DoorSide::Left if door.tile.x + 1 < level.width => {
                emit_door_position(lines, level, indent, door, door.tile.y, door.tile.x + 1);
            }
            _ => {}
        }
    }
}

fn emit_door_vertical_check(
    lines: &mut Vec<String>,
    level: &LevelSource,
    side: DoorSide,
    indent: &str,
) {
    for door in &level.doors {
        let door_y = match side {
            DoorSide::Up if door.tile.y + 1 < level.height => door.tile.y + 1,
            DoorSide::Down if door.tile.y > 0 => door.tile.y - 1,
            _ => continue,
        };
        emit_door_position(lines, level, indent, door, door_y, door.tile.x);

        if door.tile.x > 0 {
            lines.push(format!("{indent}ifnz PLAYER_SUB_X"));
            emit_door_position(
                lines,
                level,
                &format!("{indent}  "),
                door,
                door_y,
                door.tile.x - 1,
            );
            lines.push(format!("{indent}end"));
        }
    }
}

fn emit_horizontal_check(
    name: &str,
    base_map: &PositionMap,
    overlap_down_map: &PositionMap,
    platform_maps: Option<(&PositionMap, &PositionMap)>,
    _special_x: usize,
    level: &LevelSource,
    side: DoorSide,
) -> Vec<String> {
    let mut lines = vec![
        format!("macro {name}"),
        "  clear COLLISION_BLOCKED".to_string(),
    ];
    for section in level
        .sections
        .iter()
        .filter(|section| section.kind == SectionKind::Level)
    {
        let special_x = match side {
            DoorSide::Right => section.width.saturating_sub(2),
            DoorSide::Left => 1,
            DoorSide::Up | DoorSide::Down => continue,
        };
        lines.push(format!("  ifeq CURRENT_LEVEL {}", section.level));
        lines.push(format!("    ifeq PLAYER_X {special_x}"));
        lines.push("      set COLLISION_BLOCKED 1".to_string());
        lines.push("    end".to_string());
        lines.push("  end".to_string());
    }
    emit_position_map(&mut lines, level, base_map, "  ");
    lines.push("  ifnz PLAYER_SUB_Y".to_string());
    emit_position_map(&mut lines, level, overlap_down_map, "    ");
    lines.push("  end".to_string());
    if let Some((platform_base, platform_overlap_down)) = platform_maps {
        emit_platform_side_map(&mut lines, level, platform_base, "  ");
        lines.push("  ifnz PLAYER_SUB_Y".to_string());
        emit_position_map(&mut lines, level, platform_overlap_down, "    ");
        lines.push("  end".to_string());
    }
    emit_door_horizontal_check(&mut lines, level, side, "  ");
    lines.extend(["end".to_string(), String::new()]);
    lines
}

fn emit_vertical_check(
    name: &str,
    base_map: &PositionMap,
    overlap_map: &PositionMap,
    special_y: usize,
    level: &LevelSource,
    side: DoorSide,
) -> Vec<String> {
    let mut lines = vec![
        format!("macro {name}"),
        "  clear COLLISION_BLOCKED".to_string(),
        "  ifz PLAYER_SUB_Y".to_string(),
        format!("    ifeq PLAYER_Y {special_y}"),
        "      set COLLISION_BLOCKED 1".to_string(),
        "    end".to_string(),
    ];
    emit_position_map(&mut lines, level, base_map, "    ");
    emit_door_vertical_check(&mut lines, level, side, "    ");
    emit_vertical_overlap_map(&mut lines, level, overlap_map, "    ");
    lines.extend(["  end".to_string(), "end".to_string(), String::new()]);
    lines
}

fn emit_projectile_position_map(lines: &mut Vec<String>, level: &LevelSource, indent: &str) {
    let mut map = PositionMap::new();
    for tile in &level.solids {
        map.entry(tile.y).or_default().insert(tile.x);
    }

    for (level_index, leveled_map) in leveled_position_map(level, &map).iter().rev() {
        let sw = section_width_for_level(level, *level_index).unwrap_or(0);
        lines.push(format!("{indent}ifeq CURRENT_LEVEL {level_index}"));
        for (y, xs) in leveled_map.iter().rev() {
            let inner = if sw >= 2 { sw - 2 } else { 0 };
            let gap_count = inner.saturating_sub(xs.len());
            if xs.len() > gap_count {
                lines.push(format!("{indent}  ifeq PROJECTILE_Y {y}"));
                lines.push(format!("{indent}    set COLLISION_ROW_MATCH 1"));
                for x in 1..sw.saturating_sub(1) {
                    if !xs.contains(&x) {
                        lines.push(format!("{indent}    ifeq PROJECTILE_X {x}"));
                        lines.push(format!("{indent}      clear COLLISION_ROW_MATCH"));
                        lines.push(format!("{indent}    end"));
                    }
                }
                lines.push(format!("{indent}    ifnz COLLISION_ROW_MATCH"));
                lines.push(format!("{indent}      set COLLISION_BLOCKED 1"));
                lines.push(format!("{indent}    end"));
                lines.push(format!("{indent}    clear COLLISION_ROW_MATCH"));
                lines.push(format!("{indent}  end"));
            } else {
                lines.push(format!("{indent}  ifeq PROJECTILE_Y {y}"));
                for x in xs.iter().rev() {
                    lines.push(format!("{indent}    ifeq PROJECTILE_X {x}"));
                    lines.push(format!("{indent}      set COLLISION_BLOCKED 1"));
                    lines.push(format!("{indent}    end"));
                }
                lines.push(format!("{indent}  end"));
            }
        }
        lines.push(format!("{indent}end"));
    }
}

fn emit_projectile_solid_check(level: &LevelSource) -> Vec<String> {
    let mut lines = vec![
        "macro check_projectile_solid".to_string(),
        "  clear COLLISION_BLOCKED".to_string(),
    ];
    emit_projectile_position_map(&mut lines, level, "  ");
    lines.extend(["end".to_string(), String::new()]);
    lines
}

fn emit_entity_macros(level: &LevelSource) -> Vec<String> {
    let mut lines = vec!["macro reset_coins".to_string()];
    for index in 0..level.coins.len() {
        lines.push(format!("  clear COIN_{index}_COLLECTED"));
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro reset_jetpacks".to_string(),
    ]);
    for index in 0..level.jetpacks.len() {
        lines.push(format!("  clear JETPACK_{index}_COLLECTED"));
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro check_coins".to_string(),
    ]);
    for (index, coin) in level.coins.iter().enumerate() {
        let coin_x = local_x_for_level(level, coin.level, coin.tile.x);
        lines.extend([
            format!("  ifz COIN_{index}_COLLECTED"),
            format!("    ifeq CURRENT_LEVEL {}", coin.level),
            format!("      ifeq PLAYER_Y {}", coin.tile.y),
            format!("        ifeq PLAYER_X {}", coin_x),
            format!("          set COIN_{index}_COLLECTED 1"),
            "          add SCORE 10".to_string(),
            "          call audio AUDIO_PICKUP".to_string(),
            "        end".to_string(),
            "      end".to_string(),
            "    end".to_string(),
            "  end".to_string(),
        ]);
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro check_gun".to_string(),
        "  ifz GUN_COLLECTED".to_string(),
    ]);
    for gun in &level.guns {
        let gun_x = local_x_for_level(level, gun.level, gun.tile.x);
        lines.extend([
            format!("    ifeq CURRENT_LEVEL {}", gun.level),
            format!("      ifeq PLAYER_Y {}", gun.tile.y),
            format!("        ifeq PLAYER_X {}", gun_x),
            "          set GUN_COLLECTED 1".to_string(),
            "          call audio AUDIO_PICKUP".to_string(),
            "        end".to_string(),
            "      end".to_string(),
            "    end".to_string(),
        ]);
    }
    lines.extend([
        "  end".to_string(),
        "end".to_string(),
        String::new(),
        "macro check_jetpack".to_string(),
    ]);
    for (index, jetpack) in level.jetpacks.iter().enumerate() {
        let jetpack_x = local_x_for_level(level, jetpack.level, jetpack.tile.x);
        lines.extend([
            format!("  ifz JETPACK_{index}_COLLECTED"),
            format!("    ifeq CURRENT_LEVEL {}", jetpack.level),
            format!("      ifeq PLAYER_Y {}", jetpack.tile.y),
            format!("        ifeq PLAYER_X {}", jetpack_x),
            "          set JETPACK_COLLECTED 1".to_string(),
            "          set JETPACK_FUEL JETPACK_MAX_FUEL".to_string(),
            format!("          set JETPACK_{index}_COLLECTED 1"),
            "          call audio AUDIO_PICKUP".to_string(),
            "        end".to_string(),
            "      end".to_string(),
            "    end".to_string(),
            "  end".to_string(),
        ]);
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro check_key".to_string(),
        "  ifz KEY_COLLECTED".to_string(),
    ]);
    for key in &level.keys {
        let key_x = local_x_for_level(level, key.level, key.tile.x);
        lines.extend([
            format!("    ifeq CURRENT_LEVEL {}", key.level),
            format!("      ifeq PLAYER_Y {}", key.tile.y),
            format!("        ifeq PLAYER_X {}", key_x),
            "          set KEY_COLLECTED 1".to_string(),
            "          set DOOR_OPEN 1".to_string(),
            "          call audio AUDIO_DOOR".to_string(),
            "        end".to_string(),
            "      end".to_string(),
            "    end".to_string(),
        ]);
    }
    lines.extend([
        "  end".to_string(),
        "end".to_string(),
        String::new(),
        "macro reset_enemy".to_string(),
        "  set ENEMY_TIMER ENEMY_TICK_DELAY".to_string(),
        "  set ENEMY_DIR ENEMY_START_DIR".to_string(),
        "  clear ENEMY_STEP_DONE".to_string(),
        "  clear ENEMY_DEAD".to_string(),
        "  clear ENEMY_X".to_string(),
        "  clear ENEMY_Y".to_string(),
    ]);
    for enemy in &level.enemies {
        lines.extend([
            format!("  ifeq CURRENT_LEVEL {}", enemy.level),
            format!("    set ENEMY_X ENEMY_{}_START_X", enemy.level),
            format!("    set ENEMY_Y ENEMY_{}_START_Y", enemy.level),
            "  end".to_string(),
        ]);
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro reset_flying_enemy".to_string(),
        "  clear FLYING_ENEMY_DEAD".to_string(),
        "  clear FLYING_ENEMY_DIR".to_string(),
        "  clear FLYING_ENEMY_X".to_string(),
        "  clear FLYING_ENEMY_Y".to_string(),
        "  clear FLYING_ENEMY_STEP_DONE".to_string(),
        "  clear FLYING_ENEMY2_DEAD".to_string(),
        "  clear FLYING_ENEMY2_DIR".to_string(),
        "  clear FLYING_ENEMY2_X".to_string(),
        "  clear FLYING_ENEMY2_Y".to_string(),
        "  clear FLYING_ENEMY2_STEP_DONE".to_string(),
        "  clear ENEMY_PROJ1_ACTIVE".to_string(),
        "  clear ENEMY_PROJ2_ACTIVE".to_string(),
        "  set FLYING_ENEMY_SHOOT_TIMER 150".to_string(),
        "  set FLYING_ENEMY2_SHOOT_TIMER 150".to_string(),
    ]);
    let level_count = level
        .sections
        .iter()
        .filter(|section| section.kind == SectionKind::Level)
        .count()
        .max(1);
    for lvl in 1..=level_count {
        let level_enemies: Vec<_> = level
            .flying_enemies
            .iter()
            .filter(|e| e.level == lvl)
            .collect();
        lines.push(format!("  ifeq CURRENT_LEVEL {lvl}"));
        if level_enemies.is_empty() {
            lines.push("    set FLYING_ENEMY_DEAD 1".to_string());
            lines.push("    set FLYING_ENEMY2_DEAD 1".to_string());
        } else if level_enemies.len() == 1 {
            lines.push(format!(
                "    set FLYING_ENEMY_X FLYING_ENEMY_{lvl}_0_START_X"
            ));
            lines.push(format!(
                "    set FLYING_ENEMY_Y FLYING_ENEMY_{lvl}_0_START_Y"
            ));
            lines.push("    set FLYING_ENEMY_DEAD 0".to_string());
            lines.push("    set FLYING_ENEMY2_DEAD 1".to_string());
        } else {
            lines.push(format!(
                "    set FLYING_ENEMY_X FLYING_ENEMY_{lvl}_0_START_X"
            ));
            lines.push(format!(
                "    set FLYING_ENEMY_Y FLYING_ENEMY_{lvl}_0_START_Y"
            ));
            lines.push("    set FLYING_ENEMY_DEAD 0".to_string());
            lines.push(format!(
                "    set FLYING_ENEMY2_X FLYING_ENEMY_{lvl}_1_START_X"
            ));
            lines.push(format!(
                "    set FLYING_ENEMY2_Y FLYING_ENEMY_{lvl}_1_START_Y"
            ));
            lines.push("    set FLYING_ENEMY2_DEAD 0".to_string());
        }
        lines.push("  end".to_string());
    }
    lines.extend([
        "end".to_string(),
        String::new(),
        "macro tick_enemy".to_string(),
        "  ifnz ENEMY_TIMER".to_string(),
        "    dec ENEMY_TIMER".to_string(),
        "  end".to_string(),
        "  ifz ENEMY_DEAD".to_string(),
        "    ifz ENEMY_TIMER".to_string(),
    ]);
    for enemy in &level.enemies {
        lines.extend([
            format!("      ifeq CURRENT_LEVEL {}", enemy.level),
            "        ifnz ENEMY_DIR".to_string(),
            format!("          ifeq ENEMY_X ENEMY_{}_MAX_X", enemy.level),
            "            clear ENEMY_DIR".to_string(),
            "            set ENEMY_STEP_DONE 1".to_string(),
            "          end".to_string(),
            "          ifz ENEMY_STEP_DONE".to_string(),
            "            inc ENEMY_X".to_string(),
            "            set ENEMY_STEP_DONE 1".to_string(),
            "          end".to_string(),
        ]);
        lines.extend([
            "        end".to_string(),
            "        ifz ENEMY_STEP_DONE".to_string(),
            format!("          ifeq ENEMY_X ENEMY_{}_MIN_X", enemy.level),
            "            set ENEMY_DIR 1".to_string(),
            "            set ENEMY_STEP_DONE 1".to_string(),
            "          end".to_string(),
        ]);
        lines.extend([
            "        end".to_string(),
            "        ifz ENEMY_STEP_DONE".to_string(),
            "          dec ENEMY_X".to_string(),
            "          set ENEMY_STEP_DONE 1".to_string(),
            "        end".to_string(),
            "      end".to_string(),
        ]);
    }
    lines.extend(["    end".to_string(), "  end".to_string()]);
    lines.extend([
        "  ifz FLYING_ENEMY_DEAD".to_string(),
        "    ifz ENEMY_TIMER".to_string(),
        "      ifeq FLYING_ENEMY_DIR 0".to_string(),
        "        ifz FLYING_ENEMY_STEP_DONE".to_string(),
        "          inc FLYING_ENEMY_X".to_string(),
        "          inc FLYING_ENEMY_Y".to_string(),
        "          set FLYING_ENEMY_DIR 1".to_string(),
        "          set FLYING_ENEMY_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY_DIR 1".to_string(),
        "        ifz FLYING_ENEMY_STEP_DONE".to_string(),
        "          dec FLYING_ENEMY_X".to_string(),
        "          inc FLYING_ENEMY_Y".to_string(),
        "          set FLYING_ENEMY_DIR 2".to_string(),
        "          set FLYING_ENEMY_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY_DIR 2".to_string(),
        "        ifz FLYING_ENEMY_STEP_DONE".to_string(),
        "          dec FLYING_ENEMY_X".to_string(),
        "          dec FLYING_ENEMY_Y".to_string(),
        "          set FLYING_ENEMY_DIR 3".to_string(),
        "          set FLYING_ENEMY_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY_DIR 3".to_string(),
        "        ifz FLYING_ENEMY_STEP_DONE".to_string(),
        "          inc FLYING_ENEMY_X".to_string(),
        "          dec FLYING_ENEMY_Y".to_string(),
        "          set FLYING_ENEMY_DIR 0".to_string(),
        "          set FLYING_ENEMY_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "    end".to_string(),
        "  end".to_string(),
    ]);
    lines.extend([
        "  ifz FLYING_ENEMY2_DEAD".to_string(),
        "    ifz ENEMY_TIMER".to_string(),
        "      ifeq FLYING_ENEMY2_DIR 0".to_string(),
        "        ifz FLYING_ENEMY2_STEP_DONE".to_string(),
        "          inc FLYING_ENEMY2_X".to_string(),
        "          inc FLYING_ENEMY2_Y".to_string(),
        "          set FLYING_ENEMY2_DIR 1".to_string(),
        "          set FLYING_ENEMY2_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY2_DIR 1".to_string(),
        "        ifz FLYING_ENEMY2_STEP_DONE".to_string(),
        "          dec FLYING_ENEMY2_X".to_string(),
        "          inc FLYING_ENEMY2_Y".to_string(),
        "          set FLYING_ENEMY2_DIR 2".to_string(),
        "          set FLYING_ENEMY2_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY2_DIR 2".to_string(),
        "        ifz FLYING_ENEMY2_STEP_DONE".to_string(),
        "          dec FLYING_ENEMY2_X".to_string(),
        "          dec FLYING_ENEMY2_Y".to_string(),
        "          set FLYING_ENEMY2_DIR 3".to_string(),
        "          set FLYING_ENEMY2_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "      ifeq FLYING_ENEMY2_DIR 3".to_string(),
        "        ifz FLYING_ENEMY2_STEP_DONE".to_string(),
        "          inc FLYING_ENEMY2_X".to_string(),
        "          dec FLYING_ENEMY2_Y".to_string(),
        "          set FLYING_ENEMY2_DIR 0".to_string(),
        "          set FLYING_ENEMY2_STEP_DONE 1".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "    end".to_string(),
        "  end".to_string(),
    ]);
    lines.extend([
        "  ifz ENEMY_TIMER".to_string(),
        "    set ENEMY_TIMER ENEMY_TICK_DELAY".to_string(),
        "    clear ENEMY_STEP_DONE".to_string(),
        "    clear FLYING_ENEMY_STEP_DONE".to_string(),
        "    clear FLYING_ENEMY2_STEP_DONE".to_string(),
        "  end".to_string(),
        "end".to_string(),
        String::new(),
        "macro check_projectile_enemy".to_string(),
        "  ifnz PROJECTILE_ACTIVE".to_string(),
    ]);
    lines.push("    ifz ENEMY_DEAD".to_string());
    for enemy in &level.enemies {
        lines.push(format!("      ifeq CURRENT_LEVEL {}", enemy.level));
        lines.push(format!("        ifeq PROJECTILE_Y {}", enemy.start.y));
        lines.push("          ifeq_cell ENEMY_X PROJECTILE_X".to_string());
        lines.push("            set ENEMY_DEAD 1".to_string());
        lines.push("            clear PROJECTILE_ACTIVE".to_string());
        lines.push("            add SCORE 50".to_string());
        lines.push("            call audio AUDIO_HIT".to_string());
        lines.push("          end".to_string());
        lines.push("        end".to_string());
        lines.push("      end".to_string());
    }
    lines.push("    end".to_string());

    lines.push("    ifz FLYING_ENEMY_DEAD".to_string());
    lines.push("      ifeq_cell FLYING_ENEMY_X PROJECTILE_X".to_string());
    lines.push("        ifeq_cell FLYING_ENEMY_Y PROJECTILE_Y".to_string());
    lines.push("          set FLYING_ENEMY_DEAD 1".to_string());
    lines.push("          clear PROJECTILE_ACTIVE".to_string());
    lines.push("          add SCORE 50".to_string());
    lines.push("          call audio AUDIO_HIT".to_string());
    lines.push("        end".to_string());
    lines.push("      end".to_string());
    lines.push("    end".to_string());

    lines.push("    ifz FLYING_ENEMY2_DEAD".to_string());
    lines.push("      ifeq_cell FLYING_ENEMY2_X PROJECTILE_X".to_string());
    lines.push("        ifeq_cell FLYING_ENEMY2_Y PROJECTILE_Y".to_string());
    lines.push("          set FLYING_ENEMY2_DEAD 1".to_string());
    lines.push("          clear PROJECTILE_ACTIVE".to_string());
    lines.push("          add SCORE 50".to_string());
    lines.push("          call audio AUDIO_HIT".to_string());
    lines.push("        end".to_string());
    lines.push("      end".to_string());
    lines.push("    end".to_string());

    lines.push("  end".to_string());
    lines.push("end".to_string());
    lines.extend([
        String::new(),
        "macro check_enemy_hit".to_string(),
        "  ifz GAME_WIN".to_string(),
        "    ifz GAME_DEAD".to_string(),
    ]);
    lines.push("      ifz ENEMY_DEAD".to_string());
    for enemy in &level.enemies {
        lines.push(format!("        ifeq CURRENT_LEVEL {}", enemy.level));
        lines.push(format!("          ifeq PLAYER_Y {}", enemy.start.y));
        lines.push("            ifeq_cell PLAYER_X ENEMY_X".to_string());
        lines.push("              set GAME_DEAD 1".to_string());
        lines.push("              call audio AUDIO_DEATH".to_string());
        lines.push("            end".to_string());
        lines.push("            inc PLAYER_X".to_string());
        lines.push("            ifeq_cell PLAYER_X ENEMY_X".to_string());
        for sub_x in 8..=15 {
            lines.push(format!("              ifeq PLAYER_SUB_X {sub_x}"));
            lines.push("                set GAME_DEAD 1".to_string());
            lines.push("                call audio AUDIO_DEATH".to_string());
            lines.push("              end".to_string());
        }
        lines.push("            end".to_string());
        lines.push("            dec PLAYER_X".to_string());
        lines.push("          end".to_string());
        lines.push("        end".to_string());
    }
    lines.push("      end".to_string());

    lines.push("      ifz FLYING_ENEMY_DEAD".to_string());
    lines.push("        ifeq_cell PLAYER_Y FLYING_ENEMY_Y".to_string());
    lines.push("          ifeq_cell PLAYER_X FLYING_ENEMY_X".to_string());
    lines.push("            set GAME_DEAD 1".to_string());
    lines.push("            call audio AUDIO_DEATH".to_string());
    lines.push("          end".to_string());
    lines.push("          inc PLAYER_X".to_string());
    lines.push("          ifeq_cell PLAYER_X FLYING_ENEMY_X".to_string());
    for sub_x in 8..=15 {
        lines.push(format!("            ifeq PLAYER_SUB_X {sub_x}"));
        lines.push("              set GAME_DEAD 1".to_string());
        lines.push("              call audio AUDIO_DEATH".to_string());
        lines.push("            end".to_string());
    }
    lines.push("          end".to_string());
    lines.push("          dec PLAYER_X".to_string());
    lines.push("        end".to_string());
    lines.push("      end".to_string());

    lines.push("      ifz FLYING_ENEMY2_DEAD".to_string());
    lines.push("        ifeq_cell PLAYER_Y FLYING_ENEMY2_Y".to_string());
    lines.push("          ifeq_cell PLAYER_X FLYING_ENEMY2_X".to_string());
    lines.push("            set GAME_DEAD 1".to_string());
    lines.push("            call audio AUDIO_DEATH".to_string());
    lines.push("          end".to_string());
    lines.push("          inc PLAYER_X".to_string());
    lines.push("          ifeq_cell PLAYER_X FLYING_ENEMY2_X".to_string());
    for sub_x in 8..=15 {
        lines.push(format!("            ifeq PLAYER_SUB_X {sub_x}"));
        lines.push("              set GAME_DEAD 1".to_string());
        lines.push("              call audio AUDIO_DEATH".to_string());
        lines.push("            end".to_string());
    }
    lines.push("          end".to_string());
    lines.push("          dec PLAYER_X".to_string());
    lines.push("        end".to_string());
    lines.push("      end".to_string());

    lines.push("    end".to_string());
    lines.push("  end".to_string());
    lines.push("end".to_string());
    lines.push(String::new());

    lines.extend([
        "macro check_hazards".to_string(),
        "  ifz GAME_DEAD".to_string(),
        "    ifz GAME_WIN".to_string(),
    ]);
    for hazard in level
        .decorations
        .iter()
        .filter(|d| matches!(d.kind, 'F' | 'W' | 'V'))
    {
        let hazard_x = local_x_for_level(level, hazard.level, hazard.tile.x);
        let hazard_y = hazard.tile.y;
        lines.push(format!("      ifeq CURRENT_LEVEL {}", hazard.level));

        let mut check_ys = vec![hazard_y];
        if hazard_y > 0 {
            check_ys.push(hazard_y - 1);
        }
        for cy in check_ys {
            lines.push(format!("        ifeq PLAYER_Y {cy}"));
            lines.push(format!("          ifeq PLAYER_X {hazard_x}"));
            lines.push("            set GAME_DEAD 1".to_string());
            lines.push("            call audio AUDIO_DEATH".to_string());
            lines.push("          end".to_string());
            if hazard_x > 0 {
                lines.push(format!("          ifeq PLAYER_X {}", hazard_x - 1));
                for sub_x in 8..=15 {
                    lines.push(format!("            ifeq PLAYER_SUB_X {sub_x}"));
                    lines.push("              set GAME_DEAD 1".to_string());
                    lines.push("              call audio AUDIO_DEATH".to_string());
                    lines.push("            end".to_string());
                }
                lines.push("          end".to_string());
            }
            lines.push("        end".to_string());
        }
        lines.push("      end".to_string());
    }
    lines.extend([
        "    end".to_string(),
        "  end".to_string(),
        "end".to_string(),
        String::new(),
    ]);

    let level_count = level
        .sections
        .iter()
        .filter(|section| section.kind == SectionKind::Level)
        .count()
        .max(1);
    lines.extend([
        "macro check_exit".to_string(),
        "  ifz GAME_DEAD".to_string(),
        "    ifz GAME_WIN".to_string(),
    ]);
    for door in &level.doors {
        let door_x = local_x_for_level(level, door.level, door.tile.x);
        let door_y = door.tile.y;
        lines.push(format!("      ifeq CURRENT_LEVEL {}", door.level));
        lines.push(format!("        ifeq PLAYER_X {door_x}"));
        lines.push(format!("          ifeq PLAYER_Y {door_y}"));
        lines.push("            ifnz DOOR_OPEN".to_string());
        if door.level == level_count {
            lines.push("              set GAME_WIN 1".to_string());
            lines.push("              call audio AUDIO_WIN".to_string());
        } else {
            let next_level = door.level + 1;
            let spawn =
                level_spawn(level, next_level).expect("no spawn position found for next level");
            let next_spawn_x = local_x_for_level(level, next_level, spawn.x);
            let next_spawn_y = spawn.y;
            lines.extend([
                format!("              set CURRENT_LEVEL {next_level}"),
                "              clear DOOR_OPEN".to_string(),
                "              clear KEY_COLLECTED".to_string(),
                format!("              set PLAYER_X {next_spawn_x}"),
                format!("              set PLAYER_Y {next_spawn_y}"),
                "              clear PLAYER_SUB_X".to_string(),
                "              clear PLAYER_SUB_Y".to_string(),
                "              clear PLAYER_JUMP_PHASE".to_string(),
                "              clear PLAYER_JUMP_TIMER".to_string(),
                "              clear PROJECTILE_ACTIVE".to_string(),
                "              clear PROJECTILE_X".to_string(),
                "              clear PROJECTILE_Y".to_string(),
                "              clear PROJECTILE_DIR".to_string(),
                "              call reset_coins".to_string(),
                "              call reset_enemy".to_string(),
                "              call reset_flying_enemy".to_string(),
                "              call audio AUDIO_DOOR".to_string(),
            ]);
        }
        lines.push("            end".to_string());
        lines.push("          end".to_string());
        lines.push("        end".to_string());
        lines.push("      end".to_string());
    }
    lines.extend([
        "    end".to_string(),
        "  end".to_string(),
        "end".to_string(),
        String::new(),
    ]);
    let mut check_proj = vec![
        "macro check_enemy_projectile_hit".to_string(),
        "  ifz GAME_DEAD".to_string(),
        "    ifz GAME_WIN".to_string(),
    ];

    for (active_cell, x_cell, y_cell) in &[
        ("ENEMY_PROJ1_ACTIVE", "ENEMY_PROJ1_X", "ENEMY_PROJ1_Y"),
        ("ENEMY_PROJ2_ACTIVE", "ENEMY_PROJ2_X", "ENEMY_PROJ2_Y"),
    ] {
        check_proj.extend([
            format!("      ifnz {active_cell}"),
            "        clear COLLISION_BLOCKED".to_string(),
        ]);

        check_proj.push(format!("        ifeq_cell PLAYER_X {x_cell}"));
        check_proj.push("          set COLLISION_BLOCKED 1".to_string());
        check_proj.push("        end".to_string());

        check_proj.push("        inc PLAYER_X".to_string());
        check_proj.push(format!("        ifeq_cell PLAYER_X {x_cell}"));
        for sub_x in 8..=15 {
            check_proj.push(format!("          ifeq PLAYER_SUB_X {sub_x}"));
            check_proj.push("            set COLLISION_BLOCKED 1".to_string());
            check_proj.push("          end".to_string());
        }
        check_proj.push("        end".to_string());
        check_proj.push("        dec PLAYER_X".to_string());

        check_proj.push("        ifnz COLLISION_BLOCKED".to_string());
        check_proj.push(format!("          ifeq_cell PLAYER_Y {y_cell}"));
        check_proj.push("            set GAME_DEAD 1".to_string());
        check_proj.push("            call audio AUDIO_DEATH".to_string());
        check_proj.push("          end".to_string());
        check_proj.push("        end".to_string());
        check_proj.push("        clear COLLISION_BLOCKED".to_string());
        check_proj.push("      end".to_string());
    }

    check_proj.extend([
        "    end".to_string(),
        "  end".to_string(),
        "end".to_string(),
        String::new(),
    ]);
    lines.extend(check_proj);

    let mut tick_proj = vec!["macro tick_enemy_projectiles".to_string()];

    tick_proj.extend([
        "  ifz FLYING_ENEMY_DEAD".to_string(),
        "    ifnz FLYING_ENEMY_SHOOT_TIMER".to_string(),
        "      dec FLYING_ENEMY_SHOOT_TIMER".to_string(),
        "    end".to_string(),
        "    ifz FLYING_ENEMY_SHOOT_TIMER".to_string(),
        "      set FLYING_ENEMY_SHOOT_TIMER 150".to_string(),
        "      clear ENEMY_PROJ1_SHOOT_OK".to_string(),
        "      set ENEMY_PROJ1_DIR 1".to_string(),
    ]);
    for lvl in 1..=level_count {
        let level_enemies: Vec<_> = level
            .flying_enemies
            .iter()
            .filter(|e| e.level == lvl)
            .collect();
        if let Some(enemy) = level_enemies.get(0) {
            let cx = local_x_for_level(level, lvl, enemy.tile.x);
            tick_proj.push(format!("      ifeq CURRENT_LEVEL {lvl}"));
            let min_x = cx.saturating_sub(10);
            let max_x = cx + 10;
            for x in min_x..=max_x {
                tick_proj.push(format!("        ifeq PLAYER_X {x}"));
                tick_proj.push("          set ENEMY_PROJ1_SHOOT_OK 1".to_string());
                if x < cx {
                    tick_proj.push("          clear ENEMY_PROJ1_DIR".to_string());
                }
                tick_proj.push("        end".to_string());
            }
            tick_proj.push("      end".to_string());
        }
    }
    tick_proj.extend([
        "      ifnz ENEMY_PROJ1_SHOOT_OK".to_string(),
        "        ifz ENEMY_PROJ1_ACTIVE".to_string(),
        "          set ENEMY_PROJ1_ACTIVE 1".to_string(),
        "          copy FLYING_ENEMY_X ENEMY_PROJ1_X".to_string(),
        "          copy FLYING_ENEMY_Y ENEMY_PROJ1_Y".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "    end".to_string(),
        "  end".to_string(),
    ]);

    tick_proj.extend([
        "  ifz FLYING_ENEMY2_DEAD".to_string(),
        "    ifnz FLYING_ENEMY2_SHOOT_TIMER".to_string(),
        "      dec FLYING_ENEMY2_SHOOT_TIMER".to_string(),
        "    end".to_string(),
        "    ifz FLYING_ENEMY2_SHOOT_TIMER".to_string(),
        "      set FLYING_ENEMY2_SHOOT_TIMER 150".to_string(),
        "      clear ENEMY_PROJ2_SHOOT_OK".to_string(),
        "      set ENEMY_PROJ2_DIR 1".to_string(),
    ]);
    for lvl in 1..=level_count {
        let level_enemies: Vec<_> = level
            .flying_enemies
            .iter()
            .filter(|e| e.level == lvl)
            .collect();
        if let Some(enemy) = level_enemies.get(1) {
            let cx = local_x_for_level(level, lvl, enemy.tile.x);
            tick_proj.push(format!("      ifeq CURRENT_LEVEL {lvl}"));
            let min_x = cx.saturating_sub(10);
            let max_x = cx + 10;
            for x in min_x..=max_x {
                tick_proj.push(format!("        ifeq PLAYER_X {x}"));
                tick_proj.push("          set ENEMY_PROJ2_SHOOT_OK 1".to_string());
                if x < cx {
                    tick_proj.push("          clear ENEMY_PROJ2_DIR".to_string());
                }
                tick_proj.push("        end".to_string());
            }
            tick_proj.push("      end".to_string());
        }
    }
    tick_proj.extend([
        "      ifnz ENEMY_PROJ2_SHOOT_OK".to_string(),
        "        ifz ENEMY_PROJ2_ACTIVE".to_string(),
        "          set ENEMY_PROJ2_ACTIVE 1".to_string(),
        "          copy FLYING_ENEMY2_X ENEMY_PROJ2_X".to_string(),
        "          copy FLYING_ENEMY2_Y ENEMY_PROJ2_Y".to_string(),
        "        end".to_string(),
        "      end".to_string(),
        "    end".to_string(),
        "  end".to_string(),
    ]);

    tick_proj.extend([
        "  ifnz ENEMY_PROJ_MOVE_TIMER".to_string(),
        "    dec ENEMY_PROJ_MOVE_TIMER".to_string(),
        "  end".to_string(),
        "  ifz ENEMY_PROJ_MOVE_TIMER".to_string(),
        "    set ENEMY_PROJ_MOVE_TIMER 3".to_string(),
    ]);
    for (active_cell, x_cell, y_cell, dir_cell) in &[
        (
            "ENEMY_PROJ1_ACTIVE",
            "ENEMY_PROJ1_X",
            "ENEMY_PROJ1_Y",
            "ENEMY_PROJ1_DIR",
        ),
        (
            "ENEMY_PROJ2_ACTIVE",
            "ENEMY_PROJ2_X",
            "ENEMY_PROJ2_Y",
            "ENEMY_PROJ2_DIR",
        ),
    ] {
        tick_proj.extend([
            format!("    ifnz {active_cell}"),
            format!("      ifnz {dir_cell}"),
            format!("        inc {x_cell}"),
            "      end".to_string(),
            format!("      ifz {dir_cell}"),
            format!("        ifz {x_cell}"),
            format!("          clear {active_cell}"),
            "        end".to_string(),
            format!("        ifnz {x_cell}"),
            format!("          dec {x_cell}"),
            "        end".to_string(),
            "      end".to_string(),
            format!("      ifnz {active_cell}"),
            "        copy PROJECTILE_X BACKUP_PROJ_X".to_string(),
            "        copy PROJECTILE_Y BACKUP_PROJ_Y".to_string(),
            format!("        copy {x_cell} PROJECTILE_X"),
            format!("        copy {y_cell} PROJECTILE_Y"),
            "        call check_projectile_solid".to_string(),
            "        ifnz COLLISION_BLOCKED".to_string(),
            format!("          clear {active_cell}"),
            "        end".to_string(),
            "        copy BACKUP_PROJ_X PROJECTILE_X".to_string(),
            "        copy BACKUP_PROJ_Y PROJECTILE_Y".to_string(),
            "      end".to_string(),
            "    end".to_string(),
        ]);
    }
    tick_proj.push("  end".to_string());

    tick_proj.extend(["end".to_string(), String::new()]);
    lines.extend(tick_proj);

    lines
}

fn level_spawn(level: &LevelSource, level_index: usize) -> Option<Tile> {
    let section = level
        .sections
        .iter()
        .find(|section| section.kind == SectionKind::Level && section.level == level_index)?;
    let start_x = section.start_x + 1;
    let end_x = section.start_x + section.width.saturating_sub(1);
    for x in start_x..end_x {
        for y in (1..level.height - 1).rev() {
            if level.solid_set.contains(&(x, y)) {
                continue;
            }
            if !level.solid_set.contains(&(x, y + 1)) {
                continue;
            }
            if level.decorations.iter().any(|decoration| {
                decoration.level == level_index && decoration.tile.x == x && decoration.tile.y == y
            }) {
                continue;
            }
            return Some(Tile { x, y });
        }
    }
    None
}

fn assemble_file(filename: impl AsRef<Path>) -> Result<String, String> {
    let loaded = load_lines(&project_path(filename), &mut HashSet::new())?;
    assemble_loaded_lines(&loaded)
}

fn assemble_loaded_lines(loaded: &[SourceLine]) -> Result<String, String> {
    let mut constants = HashMap::new();
    let mut constant_sources: HashMap<String, SourceLine> = HashMap::new();
    for line in loaded {
        let parts = tokenize(&line.text);
        if parts.first().map(String::as_str) == Some("const") {
            if parts.len() != 3 {
                return Err(line.error("const expects exactly 2 operands"));
            }
            let name = parts
                .get(1)
                .ok_or_else(|| line.error("invalid const line"))?;
            let value = parts
                .get(2)
                .ok_or_else(|| line.error("invalid const line"))?
                .parse::<usize>()
                .map_err(|error| {
                    line.error(format!("invalid const value `{}`: {error}", parts[2]))
                })?;
            if let Some(previous) = constant_sources.get(name) {
                return Err(line.error(format!(
                    "duplicate const `{name}`; first defined at {}",
                    previous.location()
                )));
            }
            constants.insert(name.clone(), value);
            constant_sources.insert(name.clone(), line.clone());
        }
    }

    let (lines, macros) = collect_macros(&loaded)?;
    let mut emitter = Emitter::new(constants);
    emit_lines(&lines, &macros, &mut emitter, 0)?;
    Ok(optimize_brainfuck(&emitter.output))
}

fn strip_comment(line: &str) -> String {
    line.split_once(';')
        .map_or(line, |(before, _)| before)
        .trim()
        .to_string()
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    for ch in line.chars() {
        if ch == '"' {
            quoted = !quoted;
            continue;
        }
        if ch.is_whitespace() && !quoted {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn load_lines(filename: &Path, seen: &mut HashSet<PathBuf>) -> Result<Vec<SourceLine>, String> {
    let resolved = fs::canonicalize(filename)
        .map_err(|error| format!("resolve {}: {error}", filename.display()))?;
    if !seen.insert(resolved.clone()) {
        return Ok(Vec::new());
    }

    let dir = resolved
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", resolved.display()))?;
    let text = fs::read_to_string(&resolved)
        .map_err(|error| format!("read {}: {error}", resolved.display()))?;
    let mut output = Vec::new();

    for (line_index, raw) in text.lines().enumerate() {
        let line = strip_comment(raw);
        if line.is_empty() {
            continue;
        }
        let line_number = line_index + 1;
        let parts = tokenize(&line);
        if parts.first().map(String::as_str) == Some("include") {
            if parts.len() != 2 {
                return Err(format!(
                    "include expects exactly one path at {}:{line_number}: {line}",
                    resolved.display()
                ));
            }
            let include = parts
                .get(1)
                .ok_or_else(|| format!("include missing path in {}", resolved.display()))?;
            output.extend(load_lines(&dir.join(include), seen).map_err(|error| {
                format!(
                    "include `{include}` from {}:{line_number} failed: {error}",
                    resolved.display()
                )
            })?);
        } else {
            output.push(SourceLine::new(line, resolved.clone(), line_number));
        }
    }

    Ok(output)
}

fn collect_macros(
    lines: &[SourceLine],
) -> Result<(Vec<SourceLine>, HashMap<String, MacroDef>), String> {
    let mut macros = HashMap::new();
    let mut macro_sources: HashMap<String, SourceLine> = HashMap::new();
    let mut output = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let line = &lines[index];
        let parts = tokenize(&line.text);
        if parts.first().map(String::as_str) != Some("macro") {
            output.push(line.clone());
            index += 1;
            continue;
        }

        let name = parts
            .get(1)
            .ok_or_else(|| line.error("macro missing name"))?
            .clone();
        if let Some(previous) = macro_sources.get(&name) {
            return Err(line.error(format!(
                "duplicate macro `{name}`; first defined at {}",
                previous.location()
            )));
        }
        let params = parts.iter().skip(2).cloned().collect::<Vec<_>>();
        let mut seen_params = HashSet::new();
        for param in &params {
            if !seen_params.insert(param.clone()) {
                return Err(line.error(format!("duplicate parameter `{param}` in macro `{name}`")));
            }
        }
        let mut body = Vec::new();
        let mut depth = 0usize;
        index += 1;
        while index < lines.len() {
            let body_parts = tokenize(&lines[index].text);
            match body_parts.first().map(String::as_str) {
                Some("ifnz" | "ifz" | "ifeq" | "ifneq" | "ifeq_cell" | "ifneq_cell") => depth += 1,
                Some("end") if depth == 0 => break,
                Some("end") => depth -= 1,
                _ => {}
            }
            body.push(lines[index].clone());
            index += 1;
        }
        if index >= lines.len() {
            return Err(line.error(format!("missing end for macro `{name}`")));
        }
        macro_sources.insert(name.clone(), line.clone());
        macros.insert(
            name,
            MacroDef {
                params,
                body,
                defined_at: line.clone(),
            },
        );
        index += 1;
    }

    Ok((output, macros))
}

struct Emitter {
    constants: HashMap<String, usize>,
    pointer: usize,
    output: String,
}

impl Emitter {
    fn new(constants: HashMap<String, usize>) -> Self {
        Self {
            constants,
            pointer: 0,
            output: String::new(),
        }
    }

    fn cell(&self, name: &str) -> Result<usize, String> {
        if let Ok(value) = name.parse::<usize>() {
            return Ok(value);
        }
        self.constants
            .get(name)
            .copied()
            .ok_or_else(|| format!("Unknown cell or constant: {name}"))
    }

    fn value(&self, name: &str) -> Result<usize, String> {
        if let Ok(value) = name.parse::<usize>() {
            return Ok(value);
        }
        self.constants
            .get(name)
            .copied()
            .ok_or_else(|| format!("Unknown value or constant: {name}"))
    }

    fn emit(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn move_to(&mut self, cell: usize) {
        if cell >= self.pointer {
            self.emit(&">".repeat(cell - self.pointer));
        } else {
            self.emit(&"<".repeat(self.pointer - cell));
        }
        self.pointer = cell;
    }

    fn clear(&mut self, cell: &str) -> Result<(), String> {
        let target = self.cell(cell)?;
        self.move_to(target);
        self.emit("[-]");
        Ok(())
    }

    fn inc(&mut self, cell: &str, amount: &str) -> Result<(), String> {
        let target = self.cell(cell)?;
        let value = self.value(amount)?;
        self.move_to(target);
        self.emit(&"+".repeat(value));
        Ok(())
    }

    fn dec(&mut self, cell: &str, amount: &str) -> Result<(), String> {
        let target = self.cell(cell)?;
        let value = self.value(amount)?;
        self.move_to(target);
        self.emit(&"-".repeat(value));
        Ok(())
    }

    fn set(&mut self, cell: &str, value: &str) -> Result<(), String> {
        self.clear(cell)?;
        self.inc(cell, value)
    }

    fn copy(&mut self, src: &str, dst: &str) -> Result<(), String> {
        let source = self.cell(src)?;
        let target = self.cell(dst)?;
        if source == target {
            return Ok(());
        }
        self.clear(&target.to_string())?;
        self.clear(&COPY_SCRATCH.to_string())?;
        self.move_to(source);
        self.emit("[");
        self.dec(&source.to_string(), "1")?;
        self.inc(&target.to_string(), "1")?;
        self.inc(&COPY_SCRATCH.to_string(), "1")?;
        self.move_to(source);
        self.emit("]");
        self.move_to(COPY_SCRATCH);
        self.emit("[");
        self.dec(&COPY_SCRATCH.to_string(), "1")?;
        self.inc(&source.to_string(), "1")?;
        self.move_to(COPY_SCRATCH);
        self.emit("]");
        Ok(())
    }
}

fn read_block(lines: &[SourceLine], start: usize) -> Result<(Vec<SourceLine>, usize), String> {
    let mut body = Vec::new();
    let mut depth = 0usize;

    for index in start..lines.len() {
        let parts = tokenize(&lines[index].text);
        match parts.first().map(String::as_str) {
            Some("ifnz" | "ifz" | "ifeq" | "ifneq" | "ifeq_cell" | "ifneq_cell") => depth += 1,
            Some("end") if depth == 0 => return Ok((body, index + 1)),
            Some("end") => depth -= 1,
            _ => {}
        }
        body.push(lines[index].clone());
    }

    let start_line = start.saturating_sub(1);
    if let Some(line) = lines.get(start_line) {
        Err(line.error("missing end for block"))
    } else {
        Err("missing end for block".to_string())
    }
}

fn expand_macro_body(
    name: &str,
    macro_def: &MacroDef,
    args: &[String],
    call_line: &SourceLine,
) -> Result<Vec<SourceLine>, String> {
    if args.len() != macro_def.params.len() {
        return Err(call_line.error(format!(
            "macro `{name}` expects {} argument(s), got {}; defined at {}",
            macro_def.params.len(),
            args.len(),
            macro_def.defined_at.location()
        )));
    }

    let replacements = macro_def
        .params
        .iter()
        .zip(args.iter())
        .map(|(param, arg)| (param.as_str(), arg.as_str()))
        .collect::<HashMap<_, _>>();

    Ok(macro_def
        .body
        .iter()
        .map(|line| {
            let text = tokenize(&line.text)
                .into_iter()
                .map(|token| {
                    replacements
                        .get(token.as_str())
                        .copied()
                        .unwrap_or(token.as_str())
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(" ");
            SourceLine::new(text, line.file.clone(), line.line)
        })
        .collect())
}

fn emit_lines(
    lines: &[SourceLine],
    macros: &HashMap<String, MacroDef>,
    emitter: &mut Emitter,
    control_depth: usize,
) -> Result<(), String> {
    let mut index = 0;
    while index < lines.len() {
        let line = &lines[index];
        let parts = tokenize(&line.text);
        let op = parts.first().map(String::as_str).unwrap_or("");

        match op {
            "const" => index += 1,
            "call" => {
                let name = parts
                    .get(1)
                    .ok_or_else(|| line.error("call missing macro name"))?;
                let body = macros
                    .get(name)
                    .ok_or_else(|| line.error(format!("unknown macro `{name}`")))?;
                let expanded = expand_macro_body(name, body, &parts[2..], line)?;
                emit_lines(&expanded, macros, emitter, control_depth).map_err(|error| {
                    line.error(format!("while expanding macro `{name}`: {error}"))
                })?;
                index += 1;
            }
            "clear" => {
                emitter
                    .clear(required(&parts, 1, op).map_err(|error| line.error(error))?)
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "inc" => {
                emitter
                    .inc(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        parts.get(2).map_or("1", String::as_str),
                    )
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "dec" => {
                emitter
                    .dec(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        parts.get(2).map_or("1", String::as_str),
                    )
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "set" => {
                emitter
                    .set(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        required(&parts, 2, op).map_err(|error| line.error(error))?,
                    )
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "copy" => {
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        required(&parts, 2, op).map_err(|error| line.error(error))?,
                    )
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "add" => {
                emitter
                    .inc(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        required(&parts, 2, op).map_err(|error| line.error(error))?,
                    )
                    .map_err(|error| line.error(error))?;
                index += 1;
            }
            "ifnz" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                index = next;
            }
            "ifz" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                emitter
                    .set(&control.1.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("[");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                emitter.move_to(control.1);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.1);
                emitter.emit("]");
                index = next;
            }
            "ifeq" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                emitter
                    .dec(
                        &control.0.to_string(),
                        required(&parts, 2, op).map_err(|error| line.error(error))?,
                    )
                    .map_err(|error| line.error(error))?;
                emitter
                    .set(&control.1.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("[");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                emitter.move_to(control.1);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.1);
                emitter.emit("]");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                index = next;
            }
            "ifeq_cell" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                let temp = scratch_pair(control_depth + 1)
                    .map_err(|error| line.error(error))?
                    .0;
                let cell2 = required(&parts, 2, op).map_err(|error| line.error(error))?;
                emitter
                    .copy(cell2, &temp.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(temp);
                emitter.emit("[");
                emitter
                    .dec(&temp.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter
                    .dec(&control.0.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter.move_to(temp);
                emitter.emit("]");
                emitter
                    .set(&control.1.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("[");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                emitter.move_to(control.1);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.1.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.1);
                emitter.emit("]");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                index = next;
            }
            "ifneq_cell" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                let temp = scratch_pair(control_depth + 1)
                    .map_err(|error| line.error(error))?
                    .0;
                let cell2 = required(&parts, 2, op).map_err(|error| line.error(error))?;
                emitter
                    .copy(cell2, &temp.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(temp);
                emitter.emit("[");
                emitter
                    .dec(&temp.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter
                    .dec(&control.0.to_string(), "1")
                    .map_err(|error| line.error(error))?;
                emitter.move_to(temp);
                emitter.emit("]");
                emitter.move_to(control.0);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                index = next;
            }
            "ifneq" => {
                let (body, next) = read_block(lines, index + 1)?;
                let control = scratch_pair(control_depth).map_err(|error| line.error(error))?;
                emitter
                    .copy(
                        required(&parts, 1, op).map_err(|error| line.error(error))?,
                        &control.0.to_string(),
                    )
                    .map_err(|error| line.error(error))?;
                emitter
                    .dec(
                        &control.0.to_string(),
                        required(&parts, 2, op).map_err(|error| line.error(error))?,
                    )
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("[");
                emit_lines(&body, macros, emitter, control_depth + 1)?;
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                emitter.move_to(control.0);
                emitter.emit("]");
                emitter
                    .clear(&control.0.to_string())
                    .map_err(|error| line.error(error))?;
                index = next;
            }
            "end" => return Err(line.error("unexpected end")),
            _ => return Err(line.error(format!("unknown instruction `{op}`"))),
        }
    }
    Ok(())
}

fn scratch_pair(depth: usize) -> Result<(usize, usize), String> {
    let first = CONTROL_SCRATCH_BASE + depth * 2;
    let second = first + 1;
    if second >= CONTROL_SCRATCH_LIMIT {
        return Err(format!(
            "control nesting depth {depth} exceeds available scratch cells"
        ));
    }
    Ok((first, second))
}

fn required<'a>(parts: &'a [String], index: usize, op: &str) -> Result<&'a str, String> {
    parts
        .get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{op} missing operand {index}"))
}

fn optimize_brainfuck(source: &str) -> String {
    let mut output = Vec::with_capacity(source.len());
    for byte in source.bytes().filter(|byte| BF_TOKENS.contains(byte)) {
        if output
            .last()
            .copied()
            .is_some_and(|previous| cancels(previous, byte))
        {
            output.pop();
        } else {
            output.push(byte);
        }
    }
    String::from_utf8(output).expect("BF tokens are ASCII")
}

fn cancels(left: u8, right: u8) -> bool {
    matches!(
        (left, right),
        (b'>', b'<') | (b'<', b'>') | (b'+', b'-') | (b'-', b'+')
    )
}

#[cfg(test)]
fn compile_brainfuck(source: &str) -> Result<(), String> {
    let mut stack = Vec::new();
    let mut opcode_index = 0usize;
    for byte in source.bytes() {
        if !BF_TOKENS.contains(&byte) {
            continue;
        }
        match byte {
            b'[' => stack.push(opcode_index),
            b']' => {
                stack
                    .pop()
                    .ok_or_else(|| "Unmatched closing bracket".to_string())?;
            }
            _ => {}
        }
        opcode_index += 1;
    }
    if !stack.is_empty() {
        return Err("Unmatched opening bracket".to_string());
    }
    Ok(())
}

#[derive(Clone)]
enum BfToken {
    Op(char),
    Group(Vec<BfToken>),
}

fn parse_brainfuck(input: &str) -> Result<Vec<BfToken>, String> {
    let mut stack: Vec<Vec<BfToken>> = vec![Vec::new()];
    let mut open_positions = Vec::new();
    let mut opcode_index = 0usize;

    for ch in input.chars() {
        if !BF_TOKENS.contains(&(ch as u8)) {
            continue;
        }
        match ch {
            '[' => {
                stack.push(Vec::new());
                open_positions.push(opcode_index);
            }
            ']' => {
                if stack.len() == 1 {
                    return Err(format!(
                        "Unmatched closing bracket at opcode {opcode_index}"
                    ));
                }
                let group = stack.pop().expect("nested group exists");
                open_positions.pop();
                stack
                    .last_mut()
                    .expect("root group exists")
                    .push(BfToken::Group(group));
            }
            _ => stack
                .last_mut()
                .expect("root group exists")
                .push(BfToken::Op(ch)),
        }
        opcode_index += 1;
    }

    if stack.len() > 1 {
        return Err(format!(
            "Unmatched opening bracket at opcode {}",
            open_positions.last().copied().unwrap_or_default()
        ));
    }

    Ok(stack.pop().expect("root group exists"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenState {
    Move,
    Change,
    Io,
    Default,
}

fn format_pretty_brainfuck(input: &str) -> Result<String, String> {
    Ok(pretty_print(&parse_brainfuck(input)?, 0))
}

fn pretty_print(tokens: &[BfToken], depth: usize) -> String {
    let mut result = String::new();
    let mut state = TokenState::Default;

    for token in tokens {
        match token {
            BfToken::Group(group) => {
                if state != TokenState::Default {
                    result.push('\n');
                }
                result.push_str(&tabs(depth));
                result.push_str("[\n");
                result.push_str(&format!("{}\n", pretty_print(group, depth + 1).trim_end()));
                result.push_str(&tabs(depth));
                result.push_str("]\n");
                state = TokenState::Default;
            }
            BfToken::Op(ch) => {
                let kind = token_state(*ch);
                if state == TokenState::Default {
                    result.push_str(&tabs(depth));
                } else if should_break(state, kind) {
                    result.push('\n');
                    result.push_str(&tabs(depth));
                }
                result.push(*ch);
                state = kind;
            }
        }
    }

    result
}

fn token_state(ch: char) -> TokenState {
    match ch {
        '>' | '<' => TokenState::Move,
        '+' | '-' => TokenState::Change,
        '.' | ',' => TokenState::Io,
        _ => TokenState::Default,
    }
}

fn should_break(current: TokenState, next: TokenState) -> bool {
    matches!(
        (current, next),
        (TokenState::Change, TokenState::Move)
            | (TokenState::Io, TokenState::Move | TokenState::Change)
    )
}

fn tabs(depth: usize) -> String {
    " ".repeat(depth * 4)
}

fn report_size() -> Result<String, String> {
    let size_files = [
        "runners/web/index.html",
        "runners/web/runtime.js",
        "runners/web/vm.js",
        "runners/web/_headers",
        "game/generated/level.js",
        "rom/dave.bf",
    ];
    let mut total_raw = 0u64;
    let mut total_gzip = 0u64;
    let mut output = String::new();

    for file in size_files {
        let Ok(raw) = fs::read(project_path(file)) else {
            continue;
        };
        let gzip_bytes = gzip_len(&raw)?;
        total_raw += raw.len() as u64;
        total_gzip += gzip_bytes as u64;
        output.push_str(&format!(
            "{file}: {} raw bytes, {gzip_bytes} gzip bytes\n",
            raw.len(),
        ));
    }
    if total_raw > 0 {
        output.push_str(&format!(
            "total: {total_raw} raw bytes, {total_gzip} gzip bytes\n"
        ));
    }
    Ok(output)
}

fn gzip_len(raw: &[u8]) -> Result<usize, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(raw)
        .map_err(|error| format!("gzip write: {error}"))?;
    Ok(encoder
        .finish()
        .map_err(|error| format!("gzip finish: {error}"))?
        .len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lines(lines: &[&str]) -> Vec<SourceLine> {
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| SourceLine::test(line, index + 1))
            .collect()
    }

    #[test]
    fn assembler_emits_valid_brainfuck_for_demo_rom() {
        generate_level_artifacts().expect("generate level");
        let bf = assemble_file("game/main.dasm").expect("assemble game");
        assert!(bf.bytes().all(|byte| BF_TOKENS.contains(&byte)));
        compile_brainfuck(&bf).expect("valid BF brackets");
    }

    #[test]
    fn assembler_supports_control_and_constants_fixture() {
        let bf = assemble_file("compiler/fixtures/ifx.test.dasm").expect("assemble fixture");
        compile_brainfuck(&bf).expect("valid BF brackets");
    }

    #[test]
    fn assembler_rejects_duplicate_constants() {
        let error = assemble_loaded_lines(&test_lines(&["const A 0", "const A 1", "inc A"]))
            .expect_err("duplicate const should fail");

        assert!(error.contains("duplicate const `A`"));
        assert!(error.contains("<test>:2"));
        assert!(error.contains("<test>:1"));
    }

    #[test]
    fn assembler_rejects_malformed_constants() {
        let error = assemble_loaded_lines(&test_lines(&["const A 0 1"]))
            .expect_err("extra const operand should fail");

        assert!(error.contains("const expects exactly 2 operands"));
        assert!(error.contains("<test>:1"));
    }

    #[test]
    fn include_errors_report_parent_file_and_line() {
        let fixture = project_path("compiler/target/include-error.test.dasm");
        if let Some(parent) = fixture.parent() {
            fs::create_dir_all(parent).expect("create compiler target dir");
        }
        fs::write(&fixture, "\ninclude \"missing.dasm\"\n").expect("write temp fixture");

        let error =
            load_lines(&fixture, &mut HashSet::new()).expect_err("missing fixture should fail");

        let _ = fs::remove_file(&fixture);

        assert!(error.contains("include `missing.dasm`"));
        assert!(error.contains(":2 failed"));
    }

    #[test]
    fn assembler_errors_include_source_location() {
        let error = assemble_loaded_lines(&test_lines(&["const A 0", "call nope"]))
            .expect_err("unknown macro should fail");

        assert!(error.contains("<test>:2"));
        assert!(error.contains("unknown macro `nope`"));
        assert!(error.contains("call nope"));
    }

    #[test]
    fn assembler_rejects_duplicate_macros_with_locations() {
        let error =
            assemble_loaded_lines(&test_lines(&["macro a", "end", "macro a", "end", "call a"]))
                .expect_err("duplicate macro should fail");

        assert!(error.contains("duplicate macro `a`"));
        assert!(error.contains("<test>:3"));
        assert!(error.contains("<test>:1"));
    }

    #[test]
    fn assembler_supports_compile_time_macro_arguments() {
        let bf = assemble_loaded_lines(&test_lines(&[
            "const A 0",
            "const B 1",
            "const STEP 3",
            "macro bump CELL AMOUNT",
            "inc CELL AMOUNT",
            "end",
            "call bump A STEP",
            "call bump B 2",
        ]))
        .expect("assemble macro arg fixture");

        compile_brainfuck(&bf).expect("valid BF brackets");
    }

    #[test]
    fn assembler_rejects_macro_argument_count_mismatch() {
        let error = assemble_loaded_lines(&test_lines(&[
            "const A 0",
            "macro bump CELL AMOUNT",
            "inc CELL AMOUNT",
            "end",
            "call bump A",
        ]))
        .expect_err("missing macro argument should fail");

        assert!(error.contains("macro `bump` expects 2 argument(s), got 1"));
        assert!(error.contains("<test>:5"));
        assert!(error.contains("<test>:2"));
    }

    #[test]
    fn assembler_rejects_duplicate_macro_parameters() {
        let error = assemble_loaded_lines(&test_lines(&["macro bad CELL CELL", "end"]))
            .expect_err("duplicate macro parameter should fail");

        assert!(error.contains("duplicate parameter `CELL` in macro `bad`"));
        assert!(error.contains("<test>:1"));
    }

    #[test]
    fn optimizer_cancels_adjacent_inverse_ops() {
        assert_eq!(optimize_brainfuck("++--><<>+-[+-]"), "[]");
    }

    #[test]
    fn pretty_formatter_groups_moves_and_changes() {
        assert_eq!(
            format_pretty_brainfuck(">[>>+]").expect("format"),
            ">\n[\n    >>+\n]\n"
        );
    }

    #[test]
    fn pretty_formatter_indents_nested_loops() {
        assert_eq!(
            format_pretty_brainfuck(">[>[<,]]").expect("format"),
            ">\n[\n    >\n    [\n        <,\n    ]\n]\n"
        );
    }

    #[test]
    fn size_report_stays_in_compiler() {
        let report = report_size().expect("size report");
        assert!(report.contains("total:") || report.is_empty());
    }
}
