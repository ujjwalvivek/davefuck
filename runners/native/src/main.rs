use davefuck_runner::{
    compile, init_tape, read_level, run_headless, tick_rom, CompiledRom, InputState, Level,
    COIN_BASE, CURRENT_LEVEL, DOOR_OPEN, ENEMY_DEAD, ENEMY_DIR, ENEMY_TIMER, ENEMY_X, ENEMY_Y,
    FLYING_ENEMY_DEAD, FLYING_ENEMY_DIR, FLYING_ENEMY_X, FLYING_ENEMY_Y, GAME_DEAD, GAME_WIN,
    GUN_COLLECTED, JETPACK_COLLECTED, KEY_COLLECTED, PLAYER_FACING, PLAYER_JUMP_PHASE,
    PLAYER_JUMP_TIMER, PLAYER_SUB_X, PLAYER_SUB_Y, PLAYER_X, PLAYER_Y, PROJECTILE_ACTIVE,
    PROJECTILE_DIR, PROJECTILE_X, PROJECTILE_Y, SCORE, TILE_PIXELS,
};
use pixels::{Pixels, SurfaceTexture};
use std::collections::HashSet;
use std::env;
use std::io::{self, Write};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

const STEP: Duration = Duration::from_micros(16_667);
const MAX_FRAME: Duration = Duration::from_millis(100);
const HUD_PIXELS: usize = 16;

#[derive(Clone, Copy)]
struct Color(u8, u8, u8);

const PAGE_BG: Color = Color(17, 17, 17);
const BG: Color = Color(23, 23, 23);
const BRICK_DARK: Color = Color(139, 8, 6);
const BRICK_LIGHT: Color = Color(255, 42, 29);
const SKIN: Color = Color(242, 240, 223);
const SHIRT: Color = Color(74, 160, 181);
const KEY: Color = Color(255, 216, 77);
const DOOR: Color = Color(159, 100, 50);
const ENEMY: Color = Color(210, 28, 255);
const HUD_GREEN: Color = Color(69, 243, 94);
const HUD_BLUE: Color = Color(85, 223, 255);
const HUD_GOLD: Color = Color(255, 227, 90);

fn render_terminal(level: &Level, tape: &[u8], steps: usize) -> String {
    let mut screen = vec![vec!['.'; level.width]; level.height];
    for &(x, y) in &level.solids {
        let x = x as usize;
        let y = y as usize;
        if y < screen.len() && x < screen[y].len() {
            screen[y][x] = '#';
        }
    }

    if tape[KEY_COLLECTED] == 0 {
        screen[level.key_y as usize][level.key_x as usize] = 'K';
    }
    for (index, &(coin_x, coin_y)) in level.coins.iter().enumerate() {
        if tape[COIN_BASE + index] == 0 {
            screen[coin_y as usize][coin_x as usize] = 'C';
        }
    }
    screen[level.door_y as usize][level.door_x as usize] =
        if tape[DOOR_OPEN] == 0 { 'D' } else { 'd' };
    let enemy_x = tape[ENEMY_X] as usize;
    let enemy_y = tape[ENEMY_Y] as usize;
    if enemy_y < screen.len() && enemy_x < screen[enemy_y].len() {
        screen[enemy_y][enemy_x] = 'M';
    }

    let player_x = tape[PLAYER_X] as usize;
    let player_y = tape[PLAYER_Y] as usize;
    if player_y < screen.len() && player_x < screen[player_y].len() {
        screen[player_y][player_x] = '@';
    }

    let mut output = String::new();
    output.push_str("\x1b[H");
    for row in screen {
        for tile in row {
            output.push(tile);
        }
        output.push('\n');
    }
    output.push_str(&format!(
        "PX={}:{} PY={}:{} JUMP={}:{} SCORE={} KEY={} DOOR={} DEAD={} WIN={} STEPS={}  r restarts, esc quits\n",
        tape[PLAYER_X],
        tape[PLAYER_SUB_X],
        tape[PLAYER_Y],
        tape[PLAYER_SUB_Y],
        tape[PLAYER_JUMP_PHASE],
        tape[PLAYER_JUMP_TIMER],
        tape[SCORE],
        tape[KEY_COLLECTED],
        tape[DOOR_OPEN],
        tape[GAME_DEAD],
        tape[GAME_WIN],
        steps
    ));
    output
}

fn play_terminal(rom: &CompiledRom, level: &Level) -> Result<(), String> {
    let mut tape = init_tape(level);
    let mut stdout = io::stdout();

    print!("\x1b[2J\x1b[?25l");
    stdout.flush().map_err(|error| error.to_string())?;

    loop {
        let frame_start = Instant::now();
        let input = read_keyboard_state();
        if input.quit {
            break;
        }

        let steps = tick_rom(rom, &mut tape, &input)?;
        print!("{}", render_terminal(level, &tape, steps));
        stdout.flush().map_err(|error| error.to_string())?;

        if let Some(remaining) = STEP.checked_sub(frame_start.elapsed()) {
            thread::sleep(remaining);
        }
    }

    println!("\x1b[?25h");
    stdout.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn draw_frame(
    level: &Level,
    tape: &[u8],
    frame_width: usize,
    frame_height: usize,
    frame: &mut [u8],
) {
    if frame_width == 0 || frame_height == 0 {
        return;
    }

    for pixel in frame.chunks_exact_mut(4) {
        write_pixel(pixel, PAGE_BG);
    }

    let visible_width = level.width.min(19) * TILE_PIXELS;
    let game_height = HUD_PIXELS + level.height * TILE_PIXELS;
    let scale = (frame_width / visible_width)
        .min(frame_height / game_height)
        .max(1);
    let viewport_width = visible_width * scale;
    let viewport_height = game_height * scale;
    let viewport_x = frame_width.saturating_sub(viewport_width) / 2;
    let viewport_y = frame_height.saturating_sub(viewport_height) / 2;
    let camera_x = camera_pixel_x(level, tape, visible_width);
    let current_level = tape[CURRENT_LEVEL].max(1);
    let (section_start, _) = active_level_section(level, current_level);

    fill_rect(
        frame,
        frame_width,
        viewport_x,
        viewport_y,
        viewport_width,
        viewport_height,
        BG,
    );
    draw_hud(
        frame,
        frame_width,
        viewport_x,
        viewport_y,
        visible_width * scale,
        scale,
        tape,
    );
    fill_rect(
        frame,
        frame_width,
        viewport_x,
        viewport_y + HUD_PIXELS * scale,
        visible_width * scale,
        level.height * TILE_PIXELS * scale,
        Color(5, 5, 5),
    );
    draw_inner_shadows(
        level,
        frame,
        frame_width,
        viewport_x,
        viewport_y,
        camera_x,
        visible_width,
        scale,
    );

    for &(tile_x, tile_y) in &level.solids {
        let world_x = tile_x as usize * TILE_PIXELS;
        if world_x + TILE_PIXELS < camera_x || world_x > camera_x + visible_width {
            continue;
        }
        draw_brick_tile(
            level,
            frame,
            frame_width,
            tile_x as usize,
            tile_y as usize,
            viewport_x + world_x.saturating_sub(camera_x) * scale,
            viewport_y + (HUD_PIXELS + tile_y as usize * TILE_PIXELS) * scale,
            scale,
        );
    }

    if tape[KEY_COLLECTED] == 0 {
        let (_, key_x, key_y) = active_key(level, current_level);
        let key_world_x = key_x as usize * TILE_PIXELS;
        if let Some(draw_x) =
            viewport_world_x(viewport_x, key_world_x, camera_x, visible_width, scale)
        {
            draw_key(
                frame,
                frame_width,
                draw_x,
                viewport_y + (HUD_PIXELS + key_y as usize * TILE_PIXELS) * scale,
                scale,
            );
        }
    }
    for (index, &(coin_x, coin_y)) in level.coins.iter().enumerate() {
        if tape[COIN_BASE + index] == 0 {
            let coin_world_x = coin_x as usize * TILE_PIXELS;
            if let Some(draw_x) =
                viewport_world_x(viewport_x, coin_world_x, camera_x, visible_width, scale)
            {
                draw_coin(
                    frame,
                    frame_width,
                    draw_x,
                    viewport_y + (HUD_PIXELS + coin_y as usize * TILE_PIXELS) * scale,
                    scale,
                );
            }
        }
    }
    if tape[GUN_COLLECTED] == 0 {
        for &(gun_level, gun_x, gun_y) in &level.guns {
            if gun_level != current_level {
                continue;
            }
            let gun_world_x = gun_x as usize * TILE_PIXELS;
            if let Some(draw_x) =
                viewport_world_x(viewport_x, gun_world_x, camera_x, visible_width, scale)
            {
                draw_gun_pickup(
                    frame,
                    frame_width,
                    draw_x,
                    viewport_y + (HUD_PIXELS + gun_y as usize * TILE_PIXELS) * scale,
                    scale,
                );
            }
        }
    }
    let jetpack_base = COIN_BASE + level.coins.len();
    for (index, &(jetpack_level, jetpack_x, jetpack_y)) in level.jetpacks.iter().enumerate() {
        if jetpack_level != current_level {
            continue;
        }
        if tape.get(jetpack_base + index).copied().unwrap_or(1) != 0 || tape[JETPACK_COLLECTED] != 0
        {
            continue;
        }
        let jetpack_world_x = jetpack_x as usize * TILE_PIXELS;
        if let Some(draw_x) =
            viewport_world_x(viewport_x, jetpack_world_x, camera_x, visible_width, scale)
        {
            draw_jetpack_pickup(
                frame,
                frame_width,
                draw_x,
                viewport_y + (HUD_PIXELS + jetpack_y as usize * TILE_PIXELS) * scale,
                scale,
            );
        }
    }
    let fallback_doors = [(1, level.door_x, level.door_y)];
    let doors = if level.doors.is_empty() {
        &fallback_doors[..]
    } else {
        level.doors.as_slice()
    };
    for &(door_level, door_x, door_y) in doors {
        let door_world_x = door_x as usize * TILE_PIXELS;
        if let Some(draw_x) =
            viewport_world_x(viewport_x, door_world_x, camera_x, visible_width, scale)
        {
            draw_door(
                frame,
                frame_width,
                draw_x,
                viewport_y + (HUD_PIXELS + door_y as usize * TILE_PIXELS) * scale,
                scale,
                door_level < current_level || (door_level == current_level && tape[DOOR_OPEN] != 0),
            );
        }
    }
    if tape[ENEMY_DEAD] == 0
        && level
            .enemies
            .iter()
            .any(|&(enemy_level, _, _, _, _)| enemy_level == current_level)
    {
        let enemy_x = enemy_pixel_x(level, tape, current_level) * scale;
        draw_enemy(
            frame,
            frame_width,
            viewport_x + enemy_x.saturating_sub(camera_x * scale),
            viewport_y + (HUD_PIXELS + tape[ENEMY_Y] as usize * TILE_PIXELS) * scale,
            scale,
        );
    }

    if tape[FLYING_ENEMY_DEAD] == 0
        && level
            .flying_enemies
            .iter()
            .any(|&(enemy_level, _, _)| enemy_level == current_level)
    {
        let (flying_x, flying_y) = flying_enemy_pixel(level, tape, current_level);
        if let Some(draw_x) = viewport_world_x(viewport_x, flying_x, camera_x, visible_width, scale)
        {
            draw_flying_enemy(
                frame,
                frame_width,
                draw_x,
                viewport_y + (HUD_PIXELS * scale) + flying_y.saturating_mul(scale),
                scale,
            );
        }
    }

    if tape[PROJECTILE_ACTIVE] != 0 {
        let projectile_world_x = (section_start + tape[PROJECTILE_X] as usize) * TILE_PIXELS + 14;
        if let Some(draw_x) = viewport_world_x(
            viewport_x,
            projectile_world_x,
            camera_x,
            visible_width,
            scale,
        ) {
            draw_projectile(
                frame,
                frame_width,
                draw_x,
                viewport_y + (HUD_PIXELS + tape[PROJECTILE_Y] as usize * TILE_PIXELS + 10) * scale,
                scale,
                tape[PROJECTILE_DIR] != 0,
            );
        }
    }

    let player_x = tape[PLAYER_X] as usize * TILE_PIXELS + tape[PLAYER_SUB_X] as usize;
    let player_y = tape[PLAYER_Y] as usize * TILE_PIXELS + tape[PLAYER_SUB_Y] as usize;
    draw_dave(
        frame,
        frame_width,
        viewport_x + player_x.saturating_sub(camera_x) * scale,
        viewport_y + (HUD_PIXELS + player_y) * scale,
        scale,
        tape[GAME_DEAD] != 0,
        tape[GAME_WIN] != 0,
        tape[PLAYER_FACING] != 0,
        tape[PLAYER_JUMP_PHASE] != 0,
        (tape[PLAYER_SUB_X] / 4) & 1,
    );
}

fn draw_hud(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    width: usize,
    scale: usize,
    tape: &[u8],
) {
    fill_rect(
        frame,
        frame_width,
        x,
        y,
        width,
        HUD_PIXELS * scale,
        Color(3, 3, 3),
    );
    fill_rect(
        frame,
        frame_width,
        x,
        y + (HUD_PIXELS - 3) * scale,
        width,
        scale,
        Color(244, 244, 244),
    );
    fill_rect(
        frame,
        frame_width,
        x,
        y + (HUD_PIXELS - 2) * scale,
        width,
        2 * scale,
        Color(80, 80, 80),
    );

    draw_pixel_text(
        frame,
        frame_width,
        x + 4 * scale,
        y + 3 * scale,
        "DAVE",
        HUD_GREEN,
        scale,
    );
    draw_pixel_text(
        frame,
        frame_width,
        x + 40 * scale,
        y + 3 * scale,
        &format!("SCORE {:04}", tape[SCORE]),
        Color(223, 255, 228),
        scale,
    );
    draw_pixel_text(
        frame,
        frame_width,
        x + 113 * scale,
        y + 3 * scale,
        &format!("LEVEL {:02}", tape[CURRENT_LEVEL].max(1)),
        HUD_GREEN,
        scale,
    );
    draw_pixel_text(
        frame,
        frame_width,
        x + 178 * scale,
        y + 3 * scale,
        if tape[KEY_COLLECTED] != 0 {
            "KEY YES"
        } else {
            "KEY NO"
        },
        HUD_BLUE,
        scale,
    );
    draw_pixel_text(
        frame,
        frame_width,
        x + 223 * scale,
        y + 3 * scale,
        if tape[DOOR_OPEN] != 0 {
            "DOOR OPEN"
        } else {
            "DOOR LOCK"
        },
        HUD_GOLD,
        scale,
    );
}

fn draw_pixel_text(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    text: &str,
    color: Color,
    scale: usize,
) {
    let mut cursor = x;
    for ch in text.chars() {
        draw_glyph(frame, frame_width, cursor, y, ch, color, scale);
        cursor += 4 * scale;
    }
}

fn draw_glyph(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    ch: char,
    color: Color,
    scale: usize,
) {
    let glyph = glyph(ch);
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) != 0 {
                fill_rect(
                    frame,
                    frame_width,
                    x + col * scale,
                    y + row * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }
}

fn glyph(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b010, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b111, 0b100, 0b100, 0b100, 0b111],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b111, 0b100, 0b101, 0b101, 0b111],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b111, 0b101, 0b111, 0b100, 0b100],
        'Q' => [0b111, 0b101, 0b101, 0b111, 0b001],
        'R' => [0b111, 0b101, 0b111, 0b110, 0b101],
        'S' => [0b111, 0b100, 0b111, 0b001, 0b111],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        _ => [0b000, 0b000, 0b000, 0b000, 0b000],
    }
}

fn draw_inner_shadows(
    level: &Level,
    frame: &mut [u8],
    frame_width: usize,
    viewport_x: usize,
    viewport_y: usize,
    camera_x: usize,
    visible_width: usize,
    scale: usize,
) {
    for y in 0..level.height {
        for x in 0..level.width {
            if is_solid(level, x, y) {
                continue;
            }
            let world_x = x * TILE_PIXELS;
            if world_x + TILE_PIXELS < camera_x || world_x > camera_x + visible_width {
                continue;
            }
            let px = viewport_x + world_x.saturating_sub(camera_x) * scale;
            let py = viewport_y + (HUD_PIXELS + y * TILE_PIXELS) * scale;
            if y > 0 && is_solid(level, x, y - 1) {
                fill_rect(
                    frame,
                    frame_width,
                    px,
                    py,
                    TILE_PIXELS * scale,
                    2 * scale,
                    Color(65, 0, 0),
                );
            }
            if x > 0 && is_solid(level, x - 1, y) {
                fill_rect(
                    frame,
                    frame_width,
                    px,
                    py,
                    2 * scale,
                    TILE_PIXELS * scale,
                    Color(55, 0, 0),
                );
            }
        }
    }
}

fn draw_brick_tile(
    level: &Level,
    frame: &mut [u8],
    frame_width: usize,
    tile_x: usize,
    tile_y: usize,
    x: usize,
    y: usize,
    scale: usize,
) {
    let base = match (tile_x * 7 + tile_y * 11) % 4 {
        0 => Color(216, 13, 8),
        1 => Color(224, 20, 13),
        2 => Color(207, 11, 7),
        _ => Color(226, 26, 12),
    };
    fill_rect(
        frame,
        frame_width,
        x,
        y,
        TILE_PIXELS * scale,
        TILE_PIXELS * scale,
        base,
    );
    fill_rect(
        frame,
        frame_width,
        x,
        y + 4 * scale,
        16 * scale,
        scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x,
        y + 8 * scale,
        16 * scale,
        scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x,
        y + 12 * scale,
        16 * scale,
        scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x + 8 * scale,
        y,
        scale,
        4 * scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x + 4 * scale,
        y + 4 * scale,
        scale,
        4 * scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x + 12 * scale,
        y + 8 * scale,
        scale,
        4 * scale,
        BRICK_DARK,
    );
    fill_rect(
        frame,
        frame_width,
        x + scale,
        y + scale,
        14 * scale,
        scale,
        BRICK_LIGHT,
    );

    if tile_y == 0 || !is_solid(level, tile_x, tile_y - 1) {
        fill_rect(
            frame,
            frame_width,
            x,
            y,
            TILE_PIXELS * scale,
            2 * scale,
            Color(255, 90, 72),
        );
    }
    if !is_solid(level, tile_x, tile_y + 1) {
        fill_rect(
            frame,
            frame_width,
            x,
            y + (TILE_PIXELS - 2) * scale,
            TILE_PIXELS * scale,
            2 * scale,
            Color(104, 3, 2),
        );
    }
    if tile_x == 0 || !is_solid(level, tile_x - 1, tile_y) {
        fill_rect(
            frame,
            frame_width,
            x,
            y,
            2 * scale,
            TILE_PIXELS * scale,
            Color(255, 36, 24),
        );
    }
    if !is_solid(level, tile_x + 1, tile_y) {
        fill_rect(
            frame,
            frame_width,
            x + (TILE_PIXELS - 2) * scale,
            y,
            2 * scale,
            TILE_PIXELS * scale,
            Color(111, 4, 3),
        );
    }
}

fn draw_key(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 7 * scale,
        y + 8 * scale,
        2 * scale,
        2 * scale,
        KEY,
    );
    fill_rect(
        frame,
        frame_width,
        x + 6 * scale,
        y + 10 * scale,
        4 * scale,
        4 * scale,
        KEY,
    );
    fill_rect(
        frame,
        frame_width,
        x + 7 * scale,
        y + 14 * scale,
        2 * scale,
        2 * scale,
        KEY,
    );
    fill_rect(
        frame,
        frame_width,
        x + 8 * scale,
        y + 10 * scale,
        2 * scale,
        2 * scale,
        Color(255, 243, 165),
    );
}

fn draw_coin(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 5 * scale,
        6 * scale,
        2 * scale,
        Color(255, 240, 122),
    );
    fill_rect(
        frame,
        frame_width,
        x + 4 * scale,
        y + 7 * scale,
        8 * scale,
        4 * scale,
        KEY,
    );
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 11 * scale,
        6 * scale,
        2 * scale,
        Color(214, 154, 19),
    );
    fill_rect(
        frame,
        frame_width,
        x + 6 * scale,
        y + 7 * scale,
        2 * scale,
        2 * scale,
        Color(255, 247, 189),
    );
}

fn draw_door(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize, open: bool) {
    fill_rect(
        frame,
        frame_width,
        x + 3 * scale,
        y,
        11 * scale,
        16 * scale,
        DOOR,
    );
    if open {
        fill_rect(
            frame,
            frame_width,
            x + 6 * scale,
            y + 2 * scale,
            7 * scale,
            14 * scale,
            BG,
        );
        fill_rect(
            frame,
            frame_width,
            x + 3 * scale,
            y,
            3 * scale,
            16 * scale,
            Color(207, 207, 207),
        );
        return;
    }
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 3 * scale,
        scale,
        12 * scale,
        Color(109, 60, 32),
    );
    fill_rect(
        frame,
        frame_width,
        x + 11 * scale,
        y + 3 * scale,
        scale,
        12 * scale,
        Color(109, 60, 32),
    );
    fill_rect(
        frame,
        frame_width,
        x + 12 * scale,
        y + 8 * scale,
        2 * scale,
        2 * scale,
        KEY,
    );
}

fn draw_enemy(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 3 * scale,
        y + 8 * scale,
        10 * scale,
        8 * scale,
        ENEMY,
    );
    fill_rect(
        frame,
        frame_width,
        x + 3 * scale,
        y + 14 * scale,
        3 * scale,
        2 * scale,
        Color(123, 15, 155),
    );
    fill_rect(
        frame,
        frame_width,
        x + 10 * scale,
        y + 14 * scale,
        3 * scale,
        2 * scale,
        Color(123, 15, 155),
    );
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 10 * scale,
        2 * scale,
        2 * scale,
        Color(255, 242, 255),
    );
    fill_rect(
        frame,
        frame_width,
        x + 9 * scale,
        y + 10 * scale,
        2 * scale,
        2 * scale,
        Color(255, 242, 255),
    );
}

fn draw_flying_enemy(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 4 * scale,
        y + 5 * scale,
        8 * scale,
        8 * scale,
        Color(159, 96, 44),
    );
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 6 * scale,
        6 * scale,
        2 * scale,
        Color(218, 143, 75),
    );
    fill_rect(
        frame,
        frame_width,
        x + 3 * scale,
        y + 8 * scale,
        2 * scale,
        3 * scale,
        Color(238, 238, 238),
    );
    fill_rect(
        frame,
        frame_width,
        x + 11 * scale,
        y + 8 * scale,
        2 * scale,
        3 * scale,
        Color(238, 238, 238),
    );
    fill_rect(
        frame,
        frame_width,
        x + 6 * scale,
        y + 9 * scale,
        2 * scale,
        2 * scale,
        Color(255, 31, 31),
    );
    fill_rect(
        frame,
        frame_width,
        x + 10 * scale,
        y + 9 * scale,
        2 * scale,
        2 * scale,
        Color(255, 31, 31),
    );
}

fn draw_gun_pickup(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 7 * scale,
        8 * scale,
        2 * scale,
        Color(224, 224, 224),
    );
    fill_rect(
        frame,
        frame_width,
        x + 12 * scale,
        y + 6 * scale,
        2 * scale,
        2 * scale,
        Color(255, 255, 255),
    );
    fill_rect(
        frame,
        frame_width,
        x + 7 * scale,
        y + 9 * scale,
        2 * scale,
        4 * scale,
        Color(105, 72, 40),
    );
}

fn draw_jetpack_pickup(frame: &mut [u8], frame_width: usize, x: usize, y: usize, scale: usize) {
    fill_rect(
        frame,
        frame_width,
        x + 6 * scale,
        y + 4 * scale,
        4 * scale,
        10 * scale,
        Color(49, 198, 86),
    );
    fill_rect(
        frame,
        frame_width,
        x + 10 * scale,
        y + 5 * scale,
        3 * scale,
        8 * scale,
        Color(210, 210, 210),
    );
    fill_rect(
        frame,
        frame_width,
        x + 5 * scale,
        y + 14 * scale,
        2 * scale,
        2 * scale,
        Color(255, 153, 0),
    );
}

fn draw_projectile(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    scale: usize,
    right: bool,
) {
    let muzzle_offset = if right { 0 } else { 6 * scale };
    fill_rect(
        frame,
        frame_width,
        x + muzzle_offset,
        y,
        6 * scale,
        2 * scale,
        Color(255, 255, 255),
    );
    fill_rect(
        frame,
        frame_width,
        if right { x } else { x + 9 * scale },
        y + scale,
        3 * scale,
        scale,
        Color(255, 212, 92),
    );
}

fn draw_dave(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    scale: usize,
    dead: bool,
    won: bool,
    facing_right: bool,
    jumping: bool,
    walk_frame: u8,
) {
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        4,
        2,
        8,
        2,
        Color(217, 38, 24),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        10,
        3,
        4,
        2,
        Color(217, 38, 24),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        5,
        4,
        7,
        5,
        Color(255, 215, 176),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        11,
        5,
        2,
        2,
        Color(255, 215, 176),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        10,
        5,
        1,
        1,
        Color(16, 16, 16),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        6,
        8,
        5,
        1,
        Color(123, 44, 28),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        5,
        9,
        6,
        4,
        SKIN,
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        4,
        10,
        2,
        4,
        Color(255, 215, 176),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        11,
        10,
        3,
        2,
        Color(255, 215, 176),
    );
    draw_sprite_rect(
        frame,
        frame_width,
        x,
        y,
        scale,
        facing_right,
        6,
        12,
        5,
        2,
        SHIRT,
    );

    if jumping || dead || won || walk_frame == 0 {
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            5,
            14,
            3,
            2,
            Color(27, 111, 176),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            10,
            14,
            3,
            2,
            Color(27, 111, 176),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            4,
            15,
            4,
            1,
            Color(17, 17, 17),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            10,
            15,
            5,
            1,
            Color(17, 17, 17),
        );
    } else {
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            6,
            14,
            3,
            2,
            Color(27, 111, 176),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            9,
            14,
            3,
            2,
            Color(27, 111, 176),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            5,
            15,
            4,
            1,
            Color(17, 17, 17),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            9,
            15,
            4,
            1,
            Color(17, 17, 17),
        );
    }

    if dead {
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            5,
            0,
            10,
            1,
            Color(122, 5, 5),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            6,
            3,
            2,
            2,
            Color(16, 16, 16),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            10,
            3,
            2,
            2,
            Color(16, 16, 16),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            7,
            6,
            5,
            1,
            Color(197, 31, 31),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            4,
            8,
            9,
            1,
            Color(197, 31, 31),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            5,
            9,
            7,
            1,
            Color(109, 5, 5),
        );
        draw_sprite_rect(
            frame,
            frame_width,
            x,
            y,
            scale,
            facing_right,
            4,
            14,
            10,
            2,
            Color(16, 16, 16),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_sprite_rect(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    scale: usize,
    facing_right: bool,
    col: usize,
    row: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    let draw_col = if facing_right { col } else { 16 - col - width };
    fill_rect(
        frame,
        frame_width,
        x + draw_col * scale,
        y + row * scale,
        width * scale,
        height * scale,
        color,
    );
}

fn is_solid(level: &Level, x: usize, y: usize) -> bool {
    level
        .solids
        .iter()
        .any(|&(solid_x, solid_y)| solid_x as usize == x && solid_y as usize == y)
}

fn camera_pixel_x(level: &Level, tape: &[u8], visible_width: usize) -> usize {
    let current_level = tape[CURRENT_LEVEL].max(1);
    let (section_start, section_width) = active_level_section(level, current_level);
    let player_x = tape[PLAYER_X] as usize * TILE_PIXELS + tape[PLAYER_SUB_X] as usize;
    let min_camera_x = section_start * TILE_PIXELS;
    let max_camera_x = min_camera_x + (section_width * TILE_PIXELS).saturating_sub(visible_width);
    player_x
        .saturating_sub(visible_width * 42 / 100)
        .max(min_camera_x)
        .min(max_camera_x)
}

fn active_level_section(level: &Level, current_level: u8) -> (usize, usize) {
    level
        .sections
        .iter()
        .copied()
        .find(|&(level_index, _, _)| level_index == current_level)
        .map(|(_, start_x, width)| (start_x as usize, width as usize))
        .unwrap_or((0, level.width.min(19)))
}

fn active_key(level: &Level, current_level: u8) -> (u8, u8, u8) {
    level
        .keys
        .iter()
        .copied()
        .find(|&(level_index, _, _)| level_index == current_level)
        .unwrap_or((1, level.key_x, level.key_y))
}

fn active_enemy(level: &Level, current_level: u8) -> (u8, u8, u8, u8, u8) {
    level
        .enemies
        .iter()
        .copied()
        .find(|&(level_index, _, _, _, _)| level_index == current_level)
        .unwrap_or((
            1,
            level.enemy_start_x,
            level.enemy_start_y,
            level.enemy_min_x,
            level.enemy_max_x,
        ))
}

fn enemy_pixel_x(level: &Level, tape: &[u8], current_level: u8) -> usize {
    let (_, _, _, enemy_min_x, enemy_max_x) = active_enemy(level, current_level);
    let tile_x = tape[ENEMY_X] as usize;
    let start_x = tile_x * TILE_PIXELS;
    let delay = 18usize;
    let timer = (tape[ENEMY_TIMER] as usize).min(delay);
    let progress = delay.saturating_sub(timer);
    let mut target_tile_x = tile_x;

    if tape[ENEMY_DIR] != 0 && tile_x < enemy_max_x as usize {
        target_tile_x = tile_x + 1;
    } else if tape[ENEMY_DIR] == 0 && tile_x > enemy_min_x as usize {
        target_tile_x = tile_x - 1;
    }

    let target_x = target_tile_x * TILE_PIXELS;
    (start_x * (delay - progress) + target_x * progress) / delay
}

fn flying_enemy_pixel(level: &Level, tape: &[u8], current_level: u8) -> (usize, usize) {
    let (section_start, _) = active_level_section(level, current_level);
    let tile_x = section_start + tape[FLYING_ENEMY_X] as usize;
    let tile_y = tape[FLYING_ENEMY_Y] as usize;
    let start_x = tile_x * TILE_PIXELS;
    let start_y = tile_y * TILE_PIXELS;
    let delay = 18usize;
    let timer = (tape[ENEMY_TIMER] as usize).min(delay);
    let progress = delay.saturating_sub(timer);
    let (target_tile_x, target_tile_y) = match tape[FLYING_ENEMY_DIR] {
        0 => (tile_x + 1, tile_y + 1),
        1 => (tile_x.saturating_sub(1), tile_y + 1),
        2 => (tile_x.saturating_sub(1), tile_y.saturating_sub(1)),
        _ => (tile_x + 1, tile_y.saturating_sub(1)),
    };
    let target_x = target_tile_x * TILE_PIXELS;
    let target_y = target_tile_y * TILE_PIXELS;
    (
        (start_x * (delay - progress) + target_x * progress) / delay,
        (start_y * (delay - progress) + target_y * progress) / delay,
    )
}

fn viewport_world_x(
    viewport_x: usize,
    world_x: usize,
    camera_x: usize,
    visible_width: usize,
    scale: usize,
) -> Option<usize> {
    if world_x + TILE_PIXELS < camera_x || world_x > camera_x + visible_width {
        None
    } else {
        Some(viewport_x + world_x.saturating_sub(camera_x) * scale)
    }
}

fn fill_rect(
    frame: &mut [u8],
    frame_width: usize,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    let frame_height = frame.len() / 4 / frame_width;
    let x_end = (x + width).min(frame_width);
    let y_end = (y + height).min(frame_height);

    for py in y..y_end {
        for px in x..x_end {
            let index = (py * frame_width + px) * 4;
            write_pixel(&mut frame[index..index + 4], color);
        }
    }
}

fn write_pixel(pixel: &mut [u8], color: Color) {
    pixel[0] = color.0;
    pixel[1] = color.1;
    pixel[2] = color.2;
    pixel[3] = 0xff;
}

struct WindowApp {
    rom: CompiledRom,
    level: Level,
    tape: Vec<u8>,
    pressed: HashSet<KeyCode>,
    horizontal_intent: Option<KeyCode>,
    jump_queued: bool,
    shoot_queued: bool,
    jetpack_toggle_queued: bool,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    accumulator: Duration,
    previous_time: Instant,
}

impl WindowApp {
    fn new(rom: CompiledRom, level: Level) -> Self {
        let tape = init_tape(&level);
        Self {
            rom,
            level,
            tape,
            pressed: HashSet::new(),
            horizontal_intent: None,
            jump_queued: false,
            shoot_queued: false,
            jetpack_toggle_queued: false,
            window: None,
            pixels: None,
            accumulator: Duration::ZERO,
            previous_time: Instant::now(),
        }
    }

    fn input(&self) -> InputState {
        let has_jetpack = self.tape[davefuck_runner::JETPACK_COLLECTED] != 0;
        InputState {
            left: self.horizontal_intent == Some(KeyCode::ArrowLeft),
            right: self.horizontal_intent == Some(KeyCode::ArrowRight),
            jump: if has_jetpack {
                self.pressed.contains(&KeyCode::ArrowUp)
            } else {
                self.jump_queued
            },
            down: self.pressed.contains(&KeyCode::ArrowDown),
            shoot: self.shoot_queued,
            jetpack_toggle: self.jetpack_toggle_queued,
            restart: self.pressed.contains(&KeyCode::KeyR),
            quit: false,
        }
    }

    fn handle_key(&mut self, code: KeyCode, state: ElementState, event_loop: &ActiveEventLoop) {
        match state {
            ElementState::Pressed => {
                if code == KeyCode::Escape {
                    event_loop.exit();
                    return;
                }
                let was_pressed = self.pressed.contains(&code);
                self.pressed.insert(code);
                if code == KeyCode::ArrowUp && !was_pressed {
                    self.jump_queued = true;
                }
                if code == KeyCode::Space && !was_pressed {
                    self.shoot_queued = true;
                }
                if code == KeyCode::AltLeft && !was_pressed {
                    self.jetpack_toggle_queued = true;
                }
                if code == KeyCode::ArrowLeft || code == KeyCode::ArrowRight {
                    self.horizontal_intent = Some(code);
                }
            }
            ElementState::Released => {
                self.pressed.remove(&code);
                if self.horizontal_intent == Some(code) {
                    self.horizontal_intent = if self.pressed.contains(&KeyCode::ArrowLeft) {
                        Some(KeyCode::ArrowLeft)
                    } else if self.pressed.contains(&KeyCode::ArrowRight) {
                        Some(KeyCode::ArrowRight)
                    } else {
                        None
                    };
                }
            }
        }
    }

    fn update(&mut self) -> Result<(), String> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.previous_time).min(MAX_FRAME);
        self.previous_time = now;
        self.accumulator += elapsed;

        while self.accumulator >= STEP {
            let input = self.input();
            tick_rom(&self.rom, &mut self.tape, &input)?;
            self.jump_queued = false;
            self.shoot_queued = false;
            self.jetpack_toggle_queued = false;
            self.accumulator -= STEP;
        }

        Ok(())
    }
}

impl ApplicationHandler for WindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let game_width = (self.level.width.min(19) * TILE_PIXELS) as u32;
        let game_height = (HUD_PIXELS + self.level.height * TILE_PIXELS) as u32;
        let scale = event_loop
            .primary_monitor()
            .map(|monitor| monitor.size())
            .map(|size| {
                let width_scale = size.width.saturating_sub(96) / game_width;
                let height_scale = size.height.saturating_sub(128) / game_height;
                width_scale.min(height_scale).clamp(1, 4)
            })
            .unwrap_or(3);
        let width = game_width * scale;
        let height = game_height * scale;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("Brainfuck Dave")
                        .with_inner_size(LogicalSize::new(width as f64, height as f64))
                        .with_min_inner_size(LogicalSize::new(
                            game_width as f64,
                            game_height as f64,
                        )),
                )
                .expect("create native window"),
        );
        let size = window.inner_size();
        let surface = SurfaceTexture::new(size.width, size.height, window.clone());
        let pixels = Pixels::new(size.width, size.height, surface).expect("create pixel surface");

        self.window = Some(window);
        self.pixels = Some(pixels);
        self.previous_time = Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.handle_key(code, event.state, event_loop);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(pixels) = self.pixels.as_mut() {
                    if size.width > 0 && size.height > 0 {
                        pixels
                            .resize_surface(size.width, size.height)
                            .expect("resize surface");
                        pixels
                            .resize_buffer(size.width, size.height)
                            .expect("resize buffer");
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.update() {
                    eprintln!("{error}");
                    event_loop.exit();
                    return;
                }

                if let (Some(window), Some(pixels)) = (self.window.as_ref(), self.pixels.as_mut()) {
                    let size = window.inner_size();
                    draw_frame(
                        &self.level,
                        &self.tape,
                        size.width as usize,
                        size.height as usize,
                        pixels.frame_mut(),
                    );
                    if let Err(error) = pixels.render() {
                        eprintln!("render failed: {error}");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn play_window(rom: CompiledRom, level: Level) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|error| error.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = WindowApp::new(rom, level);
    event_loop
        .run_app(&mut app)
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn key_down(vk: i32) -> bool {
    #[link(name = "user32")]
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }

    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
fn key_down(_vk: i32) -> bool {
    false
}

fn read_keyboard_state() -> InputState {
    const VK_LEFT: i32 = 0x25;
    const VK_UP: i32 = 0x26;
    const VK_RIGHT: i32 = 0x27;
    const VK_DOWN: i32 = 0x28;
    const VK_ESCAPE: i32 = 0x1b;
    const VK_MENU: i32 = 0x12;
    const VK_SPACE: i32 = 0x20;
    const VK_R: i32 = 0x52;

    let left = key_down(VK_LEFT);
    let right = key_down(VK_RIGHT);

    InputState {
        left: left && !right,
        right: right && !left,
        jump: key_down(VK_UP),
        down: key_down(VK_DOWN),
        shoot: key_down(VK_SPACE),
        jetpack_toggle: key_down(VK_MENU),
        restart: key_down(VK_R),
        quit: key_down(VK_ESCAPE),
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let terminal_mode = args.iter().any(|arg| arg == "--terminal");
    let window_mode = args.iter().any(|arg| arg == "--window" || arg == "--play");
    let rom_path = args
        .iter()
        .skip(1)
        .find(|arg| *arg != "--terminal" && *arg != "--window" && *arg != "--play")
        .cloned()
        .unwrap_or_else(|| "rom/dave.bf".to_string());

    let source = std::fs::read_to_string(&rom_path)
        .map_err(|error| format!("failed to read {rom_path}: {error}"))?;
    let rom = compile(&source)?;
    let level = read_level()?;

    if terminal_mode {
        play_terminal(&rom, &level)
    } else if window_mode {
        play_window(rom, level)
    } else {
        run_headless(&rom, &level)
    }
}
