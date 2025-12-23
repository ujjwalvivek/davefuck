use std::collections::HashMap;
use std::env;

pub const TICK_REQUESTED: usize = 0;
pub const TICK_DONE: usize = 1;
pub const INPUT_RIGHT: usize = 32;
pub const INPUT_LEFT: usize = 33;
pub const INPUT_JUMP: usize = 34;
pub const INPUT_RESTART: usize = 35;
pub const INPUT_SHOOT: usize = 36;
pub const INPUT_DOWN: usize = 37;
pub const INPUT_JETPACK_TOGGLE: usize = 43;
pub const JETPACK_FUEL: usize = 44;
pub const JETPACK_TOGGLE_DONE: usize = 45;
pub const PLAYER_X: usize = 64;
pub const PLAYER_Y: usize = 65;
pub const PLAYER_JUMP_PHASE: usize = 67;
pub const PLAYER_JUMP_TIMER: usize = 68;
pub const PLAYER_SUB_X: usize = 69;
pub const PLAYER_SUB_Y: usize = 70;
pub const KEY_COLLECTED: usize = 112;
pub const DOOR_OPEN: usize = 113;
pub const ENEMY_X: usize = 114;
pub const ENEMY_Y: usize = 115;
pub const ENEMY_DIR: usize = 116;
pub const ENEMY_TIMER: usize = 117;
pub const GAME_DEAD: usize = 119;
pub const GAME_WIN: usize = 120;
pub const GAME_STARTED: usize = 121;
pub const PLAYER_FACING: usize = 122;
pub const SCORE: usize = 123;
pub const AUDIO_EVENT: usize = 124;
pub const AUDIO_SEQ: usize = 125;
pub const CURRENT_LEVEL: usize = 126;
pub const COIN_BASE: usize = 128;
pub const JETPACK_ACTIVE: usize = 54;
pub const JETPACK_COLLECTED: usize = 55;
pub const GUN_COLLECTED: usize = 56;
pub const PROJECTILE_ACTIVE: usize = 57;
pub const PROJECTILE_X: usize = 58;
pub const PROJECTILE_Y: usize = 59;
pub const PROJECTILE_DIR: usize = 60;
pub const ENEMY_DEAD: usize = 61;
pub const FLYING_ENEMY_X: usize = 38;
pub const FLYING_ENEMY_Y: usize = 39;
pub const FLYING_ENEMY_DIR: usize = 40;
pub const FLYING_ENEMY_DEAD: usize = 41;
pub const FLYING_ENEMY2_X: usize = 47;
pub const FLYING_ENEMY2_Y: usize = 48;
pub const FLYING_ENEMY2_DIR: usize = 49;
pub const FLYING_ENEMY2_DEAD: usize = 50;
pub const ENEMY_PROJ1_ACTIVE: usize = 2;
pub const ENEMY_PROJ1_X: usize = 3;
pub const ENEMY_PROJ1_Y: usize = 4;
pub const ENEMY_PROJ1_DIR: usize = 5;
pub const ENEMY_PROJ2_ACTIVE: usize = 6;
pub const ENEMY_PROJ2_X: usize = 7;
pub const ENEMY_PROJ2_Y: usize = 8;
pub const ENEMY_PROJ2_DIR: usize = 9;
pub const FLYING_ENEMY_SHOOT_TIMER: usize = 10;
pub const FLYING_ENEMY2_SHOOT_TIMER: usize = 11;
pub const ENEMY_PROJ_MOVE_TIMER: usize = 12;
pub const ENEMY_PROJ1_SHOOT_OK: usize = 14;
pub const ENEMY_PROJ2_SHOOT_OK: usize = 15;

pub const AUDIO_JUMP: u8 = 1;
pub const AUDIO_PICKUP: u8 = 3;
pub const AUDIO_DOOR: u8 = 4;

pub const TAPE_SIZE: usize = 512;
pub const TILE_PIXELS: usize = 16;
pub const MAX_TICK_STEPS: usize = 1_000_000;

#[derive(Clone)]
pub struct Level {
    pub width: usize,
    pub height: usize,
    pub level_count: u8,
    pub solids: Vec<(u8, u8)>,
    pub player_start_x: u8,
    pub player_start_y: u8,
    pub player_ground_y: u8,
    pub key_x: u8,
    pub key_y: u8,
    pub keys: Vec<(u8, u8, u8)>,
    pub coins: Vec<(u8, u8)>,
    pub guns: Vec<(u8, u8, u8)>,
    pub jetpacks: Vec<(u8, u8, u8)>,
    pub flying_enemies: Vec<(u8, u8, u8)>,
    pub door_x: u8,
    pub door_y: u8,
    pub doors: Vec<(u8, u8, u8)>,
    pub exit_x: u8,
    pub exit_y: u8,
    pub enemy_start_x: u8,
    pub enemy_start_y: u8,
    pub enemy_min_x: u8,
    pub enemy_max_x: u8,
    pub enemies: Vec<(u8, u8, u8, u8, u8)>,
    pub sections: Vec<(u8, u8, u8)>,
    pub hazards: Vec<(u8, u8, u8)>,
}

#[derive(Debug)]
pub struct CompiledRom {
    code: Vec<u8>,
    jumps: HashMap<usize, usize>,
}

#[derive(Clone, Copy, Default)]
pub struct InputState {
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub down: bool,
    pub shoot: bool,
    pub jetpack_toggle: bool,
    pub restart: bool,
    pub quit: bool,
}

pub fn compile(source: &str) -> Result<CompiledRom, String> {
    let code: Vec<u8> = source
        .bytes()
        .filter(|byte| b"><+-.,[]".contains(byte))
        .collect();
    let mut stack = Vec::new();
    let mut jumps = HashMap::new();

    for (index, opcode) in code.iter().enumerate() {
        match opcode {
            b'[' => stack.push(index),
            b']' => {
                let open = stack
                    .pop()
                    .ok_or_else(|| format!("unmatched closing bracket at opcode {index}"))?;
                jumps.insert(open, index);
                jumps.insert(index, open);
            }
            _ => {}
        }
    }

    if let Some(open) = stack.pop() {
        return Err(format!("unmatched opening bracket at opcode {open}"));
    }

    Ok(CompiledRom { code, jumps })
}

pub fn read_level() -> Result<Level, String> {
    let source = std::fs::read_to_string("game/generated/level.meta").map_err(|error| {
        format!("failed to read game/generated/level.meta: {error}. Run `npm run asm` first.")
    })?;
    parse_level_meta(&source)
}

pub fn parse_level_meta(source: &str) -> Result<Level, String> {
    let fields: HashMap<&str, &str> = source
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();

    Ok(Level {
        width: read_u8_field(&fields, "LEVEL_WIDTH")? as usize,
        height: read_u8_field(&fields, "LEVEL_HEIGHT")? as usize,
        level_count: read_u8_field(&fields, "LEVEL_COUNT").unwrap_or(1),
        solids: read_solids_field(&fields)?,
        player_start_x: read_u8_field(&fields, "PLAYER_START_X")?,
        player_start_y: read_u8_field(&fields, "PLAYER_START_Y")?,
        player_ground_y: read_u8_field(&fields, "PLAYER_GROUND_Y")?,
        key_x: read_u8_field(&fields, "KEY_X")?,
        key_y: read_u8_field(&fields, "KEY_Y")?,
        keys: read_leveled_positions_field(&fields, "KEYS")?,
        coins: read_positions_field(&fields, "COINS")?,
        guns: read_leveled_positions_field(&fields, "GUNS")?,
        jetpacks: read_leveled_positions_field(&fields, "JETPACKS")?,
        flying_enemies: read_leveled_positions_field(&fields, "FLYING_ENEMIES")?,
        door_x: read_u8_field(&fields, "DOOR_X")?,
        door_y: read_u8_field(&fields, "DOOR_Y")?,
        doors: read_leveled_positions_field(&fields, "DOORS")?,
        exit_x: read_u8_field(&fields, "EXIT_X")?,
        exit_y: read_u8_field(&fields, "EXIT_Y")?,
        enemy_start_x: read_u8_field(&fields, "ENEMY_START_X")?,
        enemy_start_y: read_u8_field(&fields, "ENEMY_START_Y")?,
        enemy_min_x: read_u8_field(&fields, "ENEMY_MIN_X")?,
        enemy_max_x: read_u8_field(&fields, "ENEMY_MAX_X")?,
        enemies: read_enemies_field(&fields, "ENEMIES")?,
        sections: read_sections_field(&fields)?,
        hazards: read_leveled_positions_field(&fields, "HAZARDS")?,
    })
}

fn read_u8_field(fields: &HashMap<&str, &str>, name: &str) -> Result<u8, String> {
    fields
        .get(name)
        .ok_or_else(|| format!("game/generated/level.meta is missing {name}"))?
        .parse::<u8>()
        .map_err(|error| format!("invalid {name} in game/generated/level.meta: {error}"))
}

fn read_solids_field(fields: &HashMap<&str, &str>) -> Result<Vec<(u8, u8)>, String> {
    read_positions_field(fields, "LEVEL_SOLIDS")
}

fn read_positions_field(fields: &HashMap<&str, &str>, name: &str) -> Result<Vec<(u8, u8)>, String> {
    let Some(value) = fields.get(name) else {
        return Err(format!("game/generated/level.meta is missing {name}"));
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }

    value
        .split(';')
        .map(|pair| {
            let (x, y) = pair.split_once(',').ok_or_else(|| {
                format!("invalid position `{pair}` in {name} in game/generated/level.meta")
            })?;
            let x = x
                .parse::<u8>()
                .map_err(|error| format!("invalid {name} x `{x}`: {error}"))?;
            let y = y
                .parse::<u8>()
                .map_err(|error| format!("invalid {name} y `{y}`: {error}"))?;
            Ok((x, y))
        })
        .collect()
}

fn read_leveled_positions_field(
    fields: &HashMap<&str, &str>,
    name: &str,
) -> Result<Vec<(u8, u8, u8)>, String> {
    let Some(value) = fields.get(name) else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }

    value
        .split(';')
        .map(|triple| {
            let parts = triple.split(',').collect::<Vec<_>>();
            if parts.len() != 3 {
                return Err(format!("invalid leveled position `{triple}` in {name}"));
            }
            Ok((
                parts[0]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} level `{}`: {error}", parts[0]))?,
                parts[1]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} x `{}`: {error}", parts[1]))?,
                parts[2]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} y `{}`: {error}", parts[2]))?,
            ))
        })
        .collect()
}

fn read_enemies_field(
    fields: &HashMap<&str, &str>,
    name: &str,
) -> Result<Vec<(u8, u8, u8, u8, u8)>, String> {
    let Some(value) = fields.get(name) else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }

    value
        .split(';')
        .map(|entry| {
            let parts = entry.split(',').collect::<Vec<_>>();
            if parts.len() != 5 {
                return Err(format!("invalid enemy `{entry}` in {name}"));
            }
            Ok((
                parts[0]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} level `{}`: {error}", parts[0]))?,
                parts[1]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} x `{}`: {error}", parts[1]))?,
                parts[2]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} y `{}`: {error}", parts[2]))?,
                parts[3]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} min x `{}`: {error}", parts[3]))?,
                parts[4]
                    .parse::<u8>()
                    .map_err(|error| format!("invalid {name} max x `{}`: {error}", parts[4]))?,
            ))
        })
        .collect()
}

fn read_sections_field(fields: &HashMap<&str, &str>) -> Result<Vec<(u8, u8, u8)>, String> {
    let Some(value) = fields.get("SECTIONS") else {
        return Ok(Vec::new());
    };
    if value.is_empty() {
        return Ok(Vec::new());
    }

    value
        .split(';')
        .filter_map(|entry| {
            let parts = entry.split(',').collect::<Vec<_>>();
            if parts.len() != 4 || parts[0] != "level" {
                return None;
            }
            Some((parts[1], parts[2], parts[3]))
        })
        .map(|(level, start_x, width)| {
            Ok((
                level
                    .parse::<u8>()
                    .map_err(|error| format!("invalid SECTIONS level `{level}`: {error}"))?,
                start_x
                    .parse::<u8>()
                    .map_err(|error| format!("invalid SECTIONS start `{start_x}`: {error}"))?,
                width
                    .parse::<u8>()
                    .map_err(|error| format!("invalid SECTIONS width `{width}`: {error}"))?,
            ))
        })
        .collect()
}

pub fn init_tape(level: &Level) -> Vec<u8> {
    let mut tape = vec![0u8; TAPE_SIZE];
    tape[PLAYER_X] = level.player_start_x;
    tape[PLAYER_Y] = level.player_start_y;
    tape[PLAYER_FACING] = 1;
    tape[ENEMY_X] = level.enemy_start_x;
    tape[ENEMY_Y] = level.enemy_start_y;
    tape[ENEMY_DIR] = 1;
    tape[CURRENT_LEVEL] = 1;

    let start_x = level
        .sections
        .iter()
        .find(|&&(l, _, _)| l == 1)
        .map(|&(_, sx, _)| sx)
        .unwrap_or(0);

    let lvl_enemies: Vec<_> = level.flying_enemies.iter().filter(|e| e.0 == 1).collect();
    if lvl_enemies.is_empty() {
        tape[FLYING_ENEMY_DEAD] = 1;
        tape[FLYING_ENEMY2_DEAD] = 1;
    } else if lvl_enemies.len() == 1 {
        let e = lvl_enemies[0];
        tape[FLYING_ENEMY_X] = e.1.saturating_sub(start_x);
        tape[FLYING_ENEMY_Y] = e.2;
        tape[FLYING_ENEMY_DEAD] = 0;
        tape[FLYING_ENEMY2_DEAD] = 1;
    } else {
        let e1 = lvl_enemies[0];
        let e2 = lvl_enemies[1];
        tape[FLYING_ENEMY_X] = e1.1.saturating_sub(start_x);
        tape[FLYING_ENEMY_Y] = e1.2;
        tape[FLYING_ENEMY_DEAD] = 0;
        tape[FLYING_ENEMY2_X] = e2.1.saturating_sub(start_x);
        tape[FLYING_ENEMY2_Y] = e2.2;
        tape[FLYING_ENEMY2_DEAD] = 0;
    }
    tape[FLYING_ENEMY_SHOOT_TIMER] = 150;
    tape[FLYING_ENEMY2_SHOOT_TIMER] = 150;

    tape
}

enum StopCondition {
    Halt,
    TickDone,
}

impl StopCondition {
    fn reached(&self, pc: usize, code_len: usize, tape: &[u8]) -> bool {
        match self {
            Self::Halt => pc >= code_len,
            Self::TickDone => tape[TICK_DONE] == 1,
        }
    }
}

fn run_brainfuck(
    rom: &CompiledRom,
    tape: &mut [u8],
    max_steps: usize,
    stop_condition: StopCondition,
) -> Result<(usize, bool), String> {
    let mut pointer = 0usize;
    let mut pc = 0usize;
    let mut steps = 0usize;

    while pc < rom.code.len() && !stop_condition.reached(pc, rom.code.len(), tape) {
        if steps >= max_steps {
            return Ok((steps, false));
        }

        match rom.code[pc] {
            b'>' => {
                pointer += 1;
                if pointer >= tape.len() {
                    return Err("tape pointer moved past end".to_string());
                }
            }
            b'<' => {
                pointer = pointer
                    .checked_sub(1)
                    .ok_or_else(|| "tape pointer moved before start".to_string())?;
            }
            b'+' => tape[pointer] = tape[pointer].wrapping_add(1),
            b'-' => tape[pointer] = tape[pointer].wrapping_sub(1),
            b'[' => {
                if tape[pointer] == 0 {
                    pc = *rom
                        .jumps
                        .get(&pc)
                        .ok_or_else(|| "missing jump target".to_string())?;
                }
            }
            b']' => {
                if tape[pointer] != 0 {
                    pc = *rom
                        .jumps
                        .get(&pc)
                        .ok_or_else(|| "missing jump target".to_string())?;
                }
            }
            b'.' | b',' => {}
            _ => {}
        }

        pc += 1;
        steps += 1;
    }

    Ok((steps, stop_condition.reached(pc, rom.code.len(), tape)))
}

pub fn run_to_halt(rom: &CompiledRom, tape: &mut [u8], max_steps: usize) -> Result<usize, String> {
    let (steps, reached) = run_brainfuck(rom, tape, max_steps, StopCondition::Halt)?;
    if !reached {
        return Err(format!("VM step limit exceeded: {max_steps}"));
    }
    Ok(steps)
}

pub fn run_until_done(rom: &CompiledRom, tape: &mut [u8]) -> Result<usize, String> {
    let (steps, reached) = run_brainfuck(rom, tape, MAX_TICK_STEPS, StopCondition::TickDone)?;

    if tape[TICK_DONE] != 1 {
        return Err(format!(
            "tick did not finish within {MAX_TICK_STEPS} BF steps"
        ));
    }

    if !reached {
        return Err(format!(
            "tick did not finish within {MAX_TICK_STEPS} BF steps"
        ));
    }

    Ok(steps)
}

pub fn tick_rom(rom: &CompiledRom, tape: &mut [u8], input: &InputState) -> Result<usize, String> {
    tape[INPUT_LEFT] = input.left as u8;
    tape[INPUT_RIGHT] = input.right as u8;
    tape[INPUT_JUMP] = input.jump as u8;
    tape[INPUT_DOWN] = input.down as u8;
    tape[INPUT_SHOOT] = input.shoot as u8;
    tape[INPUT_JETPACK_TOGGLE] = input.jetpack_toggle as u8;
    tape[INPUT_RESTART] = input.restart as u8;
    tape[TICK_DONE] = 0;
    tape[TICK_REQUESTED] = 1;
    run_until_done(rom, tape)
}

pub fn run_headless(rom: &CompiledRom, level: &Level) -> Result<(), String> {
    let ticks = env::var("TICKS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(60);
    let hold = env::var("HOLD").unwrap_or_else(|_| "right".to_string());
    let jump_tick = env::var("JUMP_TICK")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut tape = init_tape(level);
    let mut max_steps = 0usize;

    for tick in 0..ticks {
        let input = InputState {
            left: hold.split(',').any(|part| part == "left"),
            right: hold.split(',').any(|part| part == "right"),
            jump: hold.split(',').any(|part| part == "jump") || tick == jump_tick,
            down: hold.split(',').any(|part| part == "down"),
            shoot: hold.split(',').any(|part| part == "shoot"),
            jetpack_toggle: hold.split(',').any(|part| part == "jetpack"),
            restart: false,
            quit: false,
        };
        let steps = tick_rom(rom, &mut tape, &input)?;
        max_steps = max_steps.max(steps);
    }

    println!(
        "ticks={ticks} max_steps={max_steps} player={}:{} y={}:{} jump={}:{} score={} audio={}:{} key={} door={} dead={} win={}",
        tape[PLAYER_X],
        tape[PLAYER_SUB_X],
        tape[PLAYER_Y],
        tape[PLAYER_SUB_Y],
        tape[PLAYER_JUMP_PHASE],
        tape[PLAYER_JUMP_TIMER],
        tape[SCORE],
        tape[AUDIO_EVENT],
        tape[AUDIO_SEQ],
        tape[KEY_COLLECTED],
        tape[DOOR_OPEN],
        tape[GAME_DEAD],
        tape[GAME_WIN]
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_and_level() -> (CompiledRom, Level) {
        let source = std::fs::read_to_string("../../rom/dave.bf")
            .or_else(|_| std::fs::read_to_string("rom/dave.bf"))
            .expect("read rom/dave.bf; run npm run asm first");
        let meta = std::fs::read_to_string("../../game/generated/level.meta")
            .or_else(|_| std::fs::read_to_string("game/generated/level.meta"))
            .expect("read game/generated/level.meta; run npm run asm first");
        (
            compile(&source).expect("compile BF ROM"),
            parse_level_meta(&meta).expect("parse level"),
        )
    }

    fn create_tape(level: &Level, player_x: u8, player_y: u8) -> Vec<u8> {
        let mut tape = init_tape(level);
        tape[PLAYER_X] = player_x;
        tape[PLAYER_Y] = player_y;
        tape[GAME_STARTED] = 1;
        tape
    }

    fn local_x(level: &Level, target_level: u8, world_x: u8) -> u8 {
        let start_x = level
            .sections
            .iter()
            .find(|&&(l, _, _)| l == target_level)
            .map(|&(_, sx, _)| sx)
            .unwrap_or(0);
        world_x.saturating_sub(start_x)
    }

    fn pixel_x(tape: &[u8]) -> usize {
        tape[PLAYER_X] as usize * TILE_PIXELS + tape[PLAYER_SUB_X] as usize
    }

    fn pixel_y(tape: &[u8]) -> usize {
        tape[PLAYER_Y] as usize * TILE_PIXELS + tape[PLAYER_SUB_Y] as usize
    }

    fn tick(rom: &CompiledRom, tape: &mut [u8], input: InputState) -> usize {
        tick_rom(rom, tape, &input).expect("BF tick finishes")
    }

    #[test]
    fn vm_cell_increment_wraps_at_8_bits() {
        let rom = compile("+").expect("compile BF");
        let mut tape = vec![0u8; 1];
        tape[0] = 255;

        run_to_halt(&rom, &mut tape, 100).expect("run BF");

        assert_eq!(tape[0], 0);
    }

    #[test]
    fn vm_pointer_movement_and_loops_work() {
        let rom = compile("++[>+<-]").expect("compile BF");
        let mut tape = vec![0u8; 2];

        run_to_halt(&rom, &mut tape, 100).expect("run BF");

        assert_eq!(tape[0], 0);
        assert_eq!(tape[1], 2);
    }

    #[test]
    fn vm_reports_bracket_mismatch_errors() {
        assert!(compile("[++")
            .expect_err("unmatched opening bracket")
            .contains("unmatched opening bracket"));
        assert!(compile("++]")
            .expect_err("unmatched closing bracket")
            .contains("unmatched closing bracket"));
    }

    #[test]
    fn vm_reports_pointer_bounds_errors() {
        let mut tape = vec![0u8; 1];
        let before_start = compile("<").expect("compile BF");
        assert_eq!(
            run_to_halt(&before_start, &mut tape, 100).expect_err("pointer before start"),
            "tape pointer moved before start"
        );

        let past_end = compile(">").expect("compile BF");
        assert_eq!(
            run_to_halt(&past_end, &mut tape, 100).expect_err("pointer past end"),
            "tape pointer moved past end"
        );
    }

    #[test]
    fn vm_reports_step_limit_when_program_does_not_halt() {
        let rom = compile("+[]").expect("compile BF");
        let mut tape = vec![0u8; 1];

        assert_eq!(
            run_to_halt(&rom, &mut tape, 100).expect_err("step limit"),
            "VM step limit exceeded: 100"
        );
    }

    #[test]
    fn level_data_initializes_player_state_before_tick() {
        let (rom, level) = rom_and_level();
        let mut tape = init_tape(&level);
        tick(&rom, &mut tape, InputState::default());

        assert_eq!(tape[PLAYER_X], level.player_start_x);
        assert_eq!(tape[PLAYER_Y], level.player_start_y);
        assert_eq!(tape[PLAYER_SUB_X], 0);
        assert_eq!(tape[PLAYER_SUB_Y], 0);
    }

    #[test]
    fn brainfuck_moves_right_by_owned_subpixels() {
        let (rom, level) = rom_and_level();
        let mut tape = init_tape(&level);
        tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[PLAYER_X], level.player_start_x);
        assert_eq!(tape[PLAYER_SUB_X], 1);
    }

    #[test]
    fn brainfuck_crosses_horizontal_tile_after_subpixel_movement() {
        let (rom, level) = rom_and_level();
        let mut tape = init_tape(&level);

        for _ in 0..16 {
            tick(
                &rom,
                &mut tape,
                InputState {
                    right: true,
                    ..InputState::default()
                },
            );
        }

        assert_eq!(tape[PLAYER_X], level.player_start_x + 1);
        assert_eq!(tape[PLAYER_SUB_X], 0);
    }

    #[test]
    fn brainfuck_blocks_horizontal_wall_bounds() {
        let (rom, level) = rom_and_level();

        let mut blocked_left = create_tape(&level, 1, level.player_start_y);
        tick(
            &rom,
            &mut blocked_left,
            InputState {
                left: true,
                ..InputState::default()
            },
        );
        assert_eq!(blocked_left[PLAYER_X], 1);
        assert_eq!(blocked_left[PLAYER_SUB_X], 0);

        let level_1_width = level
            .sections
            .iter()
            .find(|&&(l, _, _)| l == 1)
            .map(|&(_, _, w)| w)
            .unwrap_or(level.width as u8);
        let max_x = level_1_width - 2;

        let mut blocked_right = create_tape(&level, max_x, level.player_start_y);
        tick(
            &rom,
            &mut blocked_right,
            InputState {
                right: true,
                ..InputState::default()
            },
        );
        assert_eq!(blocked_right[PLAYER_X], max_x);
        assert_eq!(blocked_right[PLAYER_SUB_X], 0);
    }

    #[test]
    fn brainfuck_blocks_horizontal_entry_into_generated_solids() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, 3, 7);
        tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[PLAYER_X], 3);
        assert_eq!(tape[PLAYER_SUB_X], 0);
    }

    #[test]
    fn brainfuck_platform_top_and_left_edge_support_are_solid() {
        let (rom, level) = rom_and_level();

        let mut platform_top = create_tape(&level, 7, 6);
        tick(&rom, &mut platform_top, InputState::default());
        assert_eq!(pixel_y(&platform_top), 6 * TILE_PIXELS);

        let mut left_edge = create_tape(&level, 6, 6);
        left_edge[PLAYER_SUB_X] = 2;
        tick(&rom, &mut left_edge, InputState::default());
        assert_eq!(pixel_y(&left_edge), 6 * TILE_PIXELS);
    }

    #[test]
    fn brainfuck_falls_through_gaps_between_platforms() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, 8, 6);
        tick(&rom, &mut tape, InputState::default());

        assert_eq!(pixel_y(&tape), 6 * TILE_PIXELS + 2);
    }

    #[test]
    fn brainfuck_blocks_upward_movement_into_platform_bottoms() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, 7, 8);
        tape[PLAYER_JUMP_PHASE] = 1;
        tape[PLAYER_JUMP_TIMER] = 1;
        tick(&rom, &mut tape, InputState::default());

        assert_eq!(pixel_y(&tape), 8 * TILE_PIXELS);
        assert_eq!(tape[PLAYER_JUMP_PHASE], 2);
    }

    #[test]
    fn brainfuck_jump_forms_vertical_pixel_arc() {
        let (rom, level) = rom_and_level();
        let jump_start_x = 2;
        let mut tape = create_tape(&level, jump_start_x, level.player_ground_y);
        let start_y = level.player_ground_y as usize * TILE_PIXELS;
        let start_x = jump_start_x as usize * TILE_PIXELS;

        tick(
            &rom,
            &mut tape,
            InputState {
                jump: true,
                ..InputState::default()
            },
        );
        assert_eq!(pixel_y(&tape), start_y - 2);

        for _ in 0..20 {
            tick(&rom, &mut tape, InputState::default());
        }
        assert_eq!(pixel_y(&tape), start_y - 42);
        assert_eq!(pixel_x(&tape), start_x);
        assert_eq!(tape[PLAYER_JUMP_PHASE], 2);

        for _ in 0..22 {
            tick(&rom, &mut tape, InputState::default());
        }
        assert_eq!(pixel_y(&tape), start_y);
        assert_eq!(pixel_x(&tape), start_x);
        assert_eq!(tape[PLAYER_JUMP_PHASE], 0);
    }

    #[test]
    fn brainfuck_applies_horizontal_input_while_airborne() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.player_start_x, level.player_ground_y);
        let start_y = level.player_ground_y as usize * TILE_PIXELS;
        let start_x = level.player_start_x as usize * TILE_PIXELS;

        tick(
            &rom,
            &mut tape,
            InputState {
                jump: true,
                ..InputState::default()
            },
        );
        for _ in 0..8 {
            tick(
                &rom,
                &mut tape,
                InputState {
                    right: true,
                    ..InputState::default()
                },
            );
        }

        assert!(pixel_y(&tape) < start_y);
        assert!(pixel_x(&tape) > start_x);
        assert_eq!(tape[TICK_DONE], 1);
    }

    #[test]
    fn brainfuck_updates_player_facing_for_renderer() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.player_start_x, level.player_start_y);

        tick(
            &rom,
            &mut tape,
            InputState {
                left: true,
                ..InputState::default()
            },
        );
        assert_eq!(tape[PLAYER_FACING], 0);

        tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                ..InputState::default()
            },
        );
        assert_eq!(tape[PLAYER_FACING], 1);
    }

    #[test]
    fn brainfuck_diagonal_jump_finishes_within_native_budget() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.width as u8 - 3, level.player_start_y);
        let steps = tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                jump: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[TICK_DONE], 1);
        assert!(steps < MAX_TICK_STEPS);
    }

    #[test]
    fn brainfuck_sandwiched_vertical_collision_finishes_within_budget() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, 10, 6);
        tape[PLAYER_SUB_X] = 2;
        tape[PLAYER_SUB_Y] = 0;
        tape[PLAYER_JUMP_PHASE] = 2;

        let steps = tick(&rom, &mut tape, InputState::default());

        assert_eq!(tape[TICK_DONE], 1);
        assert!(steps < MAX_TICK_STEPS);
    }

    #[test]
    fn brainfuck_collects_key_and_opens_door() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.key_x, level.key_y);

        tick(&rom, &mut tape, InputState::default());

        assert_eq!(tape[KEY_COLLECTED], 1);
        assert_eq!(tape[DOOR_OPEN], 1);
        assert_eq!(tape[AUDIO_EVENT], AUDIO_DOOR);
    }

    #[test]
    fn brainfuck_collects_coins_and_updates_score() {
        let (rom, level) = rom_and_level();
        let (coin_x, coin_y) = level.coins[0];
        let mut tape = create_tape(&level, coin_x, coin_y);

        tick(&rom, &mut tape, InputState::default());

        assert_eq!(tape[COIN_BASE], 1);
        assert_eq!(tape[SCORE], 10);
        assert_eq!(tape[AUDIO_EVENT], AUDIO_PICKUP);
        assert_eq!(tape[AUDIO_SEQ], 1);

        tick(&rom, &mut tape, InputState::default());

        assert_eq!(tape[COIN_BASE], 1);
        assert_eq!(tape[SCORE], 10);
        assert_eq!(tape[AUDIO_SEQ], 1);
    }

    #[test]
    fn brainfuck_treats_closed_door_as_collision() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.door_x - 1, level.door_y);

        tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[PLAYER_X], level.door_x - 1);
        assert_eq!(tape[PLAYER_SUB_X], 0);
    }

    #[test]
    fn brainfuck_allows_open_door_entry() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.door_x - 1, level.door_y);
        tape[DOOR_OPEN] = 1;

        tick(
            &rom,
            &mut tape,
            InputState {
                right: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[PLAYER_X], level.door_x - 1);
        assert_eq!(tape[PLAYER_SUB_X], 1);
    }

    #[test]
    fn brainfuck_sets_win_only_at_exit_after_door_opens() {
        let (rom, level) = rom_and_level();
        let mut locked = create_tape(&level, level.exit_x, level.exit_y);
        tick(&rom, &mut locked, InputState::default());
        assert_eq!(locked[GAME_WIN], 0);

        let mut level_one_open = create_tape(&level, level.exit_x, level.exit_y);
        level_one_open[DOOR_OPEN] = 1;
        tick(&rom, &mut level_one_open, InputState::default());
        assert_eq!(level_one_open[GAME_WIN], 0);
        assert_eq!(level_one_open[CURRENT_LEVEL], 2);
        assert_eq!(level_one_open[DOOR_OPEN], 0);

        let final_door = level
            .doors
            .iter()
            .copied()
            .find(|&(door_level, _, _)| door_level == level.level_count)
            .expect("final door");
        let local_door_x = local_x(&level, level.level_count, final_door.1);
        let mut final_open = create_tape(&level, local_door_x, final_door.2);
        final_open[CURRENT_LEVEL] = level.level_count;
        final_open[DOOR_OPEN] = 1;
        tick(&rom, &mut final_open, InputState::default());
        assert_eq!(final_open[GAME_WIN], 1);
    }

    #[test]
    fn brainfuck_enemy_patrols_and_kills_player() {
        let (rom, level) = rom_and_level();
        let Some(&(enemy_level, enemy_x, enemy_y, enemy_min_x, enemy_max_x)) =
            level.enemies.first()
        else {
            return;
        };

        let mut patrol = create_tape(&level, level.player_start_x, level.player_start_y);
        patrol[CURRENT_LEVEL] = enemy_level;
        patrol[ENEMY_X] = enemy_x;
        patrol[ENEMY_Y] = enemy_y;
        tick(&rom, &mut patrol, InputState::default());
        assert_eq!(
            patrol[ENEMY_X],
            (enemy_x + 1).min(enemy_max_x).max(enemy_min_x)
        );
        assert_eq!(patrol[ENEMY_Y], enemy_y);

        let mut hit = create_tape(&level, enemy_x, enemy_y);
        hit[CURRENT_LEVEL] = enemy_level;
        hit[ENEMY_X] = enemy_x;
        hit[ENEMY_Y] = enemy_y;
        tick(&rom, &mut hit, InputState::default());
        assert_eq!(hit[GAME_DEAD], 1);
    }

    #[test]
    fn brainfuck_fire_and_water_kill_player() {
        let (rom, level) = rom_and_level();
        let Some(&(hazard_level, hazard_x, hazard_y)) = level.hazards.first() else {
            return;
        };

        let local_hazard_x = local_x(&level, hazard_level, hazard_x);
        let mut direct_hit = create_tape(&level, local_hazard_x, hazard_y);
        direct_hit[CURRENT_LEVEL] = hazard_level;
        tick(&rom, &mut direct_hit, InputState::default());
        assert_eq!(direct_hit[GAME_DEAD], 1);

        let mut foot_hit = create_tape(&level, local_hazard_x, hazard_y - 1);
        foot_hit[CURRENT_LEVEL] = hazard_level;
        tick(&rom, &mut foot_hit, InputState::default());
        assert_eq!(foot_hit[GAME_DEAD], 1);
    }

    #[test]
    fn brainfuck_restart_resets_v1_game_state() {
        let (rom, level) = rom_and_level();
        let mut tape = create_tape(&level, level.exit_x, level.exit_y);
        tape[PLAYER_SUB_X] = 7;
        tape[PLAYER_SUB_Y] = 6;
        tape[KEY_COLLECTED] = 1;
        tape[DOOR_OPEN] = 1;
        tape[SCORE] = 50;
        tape[COIN_BASE] = 1;
        tape[GAME_DEAD] = 1;
        tape[GAME_WIN] = 1;
        tape[ENEMY_X] = level.enemy_max_x;

        tick(
            &rom,
            &mut tape,
            InputState {
                restart: true,
                ..InputState::default()
            },
        );

        assert_eq!(tape[PLAYER_X], level.player_start_x);
        assert_eq!(tape[PLAYER_Y], level.player_start_y);
        assert_eq!(tape[PLAYER_SUB_X], 0);
        assert_eq!(tape[PLAYER_SUB_Y], 0);
        assert_eq!(tape[KEY_COLLECTED], 0);
        assert_eq!(tape[DOOR_OPEN], 0);
        assert_eq!(tape[SCORE], 0);
        assert_eq!(tape[COIN_BASE], 0);
        assert_eq!(tape[GAME_DEAD], 0);
        assert_eq!(tape[GAME_WIN], 0);
        assert_eq!(tape[ENEMY_X], level.enemy_start_x);
        assert_eq!(tape[ENEMY_Y], level.enemy_start_y);
    }
}
