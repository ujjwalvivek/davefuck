import { BrainfuckVM, MEM } from "./vm.js";
import { level } from "/game/generated/level.js";

const TILE = 32;
const LOGICAL_TILE = 16;
const RENDER_SCALE = TILE / LOGICAL_TILE;
const HUD_HEIGHT = 32;
const SCREEN_WIDTH = level.width * TILE;
const SCREEN_HEIGHT = HUD_HEIGHT + level.height * TILE;
const STAGE_WIDTH = 640;
const STAGE_HEIGHT = 360;
const VIEW_X = Math.floor((STAGE_WIDTH - SCREEN_WIDTH) / 2);
const VIEW_Y = Math.floor((STAGE_HEIGHT - SCREEN_HEIGHT) / 2);
const STEP_MS = 1000 / 60;
const MAX_FRAME_MS = 100;

const canvas = document.querySelector("#screen");
const status = document.querySelector("#status");
const ctx = canvas.getContext("2d");
ctx.imageSmoothingEnabled = false;

const keys = new Set();
const solidTiles = new Set(level.solids.map((solid) => tileKey(solid.x, solid.y)));
const visualEnemy = { initialized: false, x: 0, y: 0 };
let horizontalIntent = null;

const FONT = Object.freeze({
  "0": ["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
  "1": ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
  "2": ["01110", "10001", "00001", "00010", "00100", "01000", "11111"],
  "3": ["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
  "4": ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
  "5": ["11111", "10000", "10000", "11110", "00001", "00001", "11110"],
  "6": ["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
  "7": ["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
  "8": ["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
  "9": ["01110", "10001", "10001", "01111", "00001", "00010", "11100"],
  A: ["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
  B: ["11110", "10001", "10001", "11110", "10001", "10001", "11110"],
  C: ["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
  D: ["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
  E: ["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
  F: ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
  G: ["01111", "10000", "10000", "10011", "10001", "10001", "01111"],
  H: ["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
  I: ["01110", "00100", "00100", "00100", "00100", "00100", "01110"],
  J: ["00111", "00010", "00010", "00010", "00010", "10010", "01100"],
  K: ["10001", "10010", "10100", "11000", "10100", "10010", "10001"],
  L: ["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
  M: ["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
  N: ["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
  O: ["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
  P: ["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
  Q: ["01110", "10001", "10001", "10001", "10101", "10010", "01101"],
  R: ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
  S: ["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
  T: ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
  U: ["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
  V: ["10001", "10001", "10001", "10001", "10001", "01010", "00100"],
  W: ["10001", "10001", "10001", "10101", "10101", "10101", "01010"],
  X: ["10001", "10001", "01010", "00100", "01010", "10001", "10001"],
  Y: ["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
  Z: ["11111", "00001", "00010", "00100", "01000", "10000", "11111"],
  ":": ["00000", "00100", "00100", "00000", "00100", "00100", "00000"],
  "-": ["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
  "/": ["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
  " ": ["00000", "00000", "00000", "00000", "00000", "00000", "00000"],
});

function resize() {
  canvas.width = STAGE_WIDTH;
  canvas.height = STAGE_HEIGHT;
  ctx.imageSmoothingEnabled = false;
}

function draw(tape) {
  ctx.fillStyle = "#080808";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  drawHud(tape);
  ctx.fillStyle = "#050505";
  ctx.fillRect(VIEW_X, VIEW_Y + HUD_HEIGHT, SCREEN_WIDTH, level.height * TILE);
  drawInnerShadows();

  for (const solid of level.solids) {
    drawBrickTile(solid.x, solid.y);
  }

  level.coins.forEach((coin, index) => {
    if (tape[MEM.COIN_BASE + index] === 0) {
      drawCoin(VIEW_X + coin.x * TILE, VIEW_Y + HUD_HEIGHT + coin.y * TILE);
    }
  });
  if (tape[MEM.KEY_COLLECTED] === 0) {
    drawKey(VIEW_X + level.key.x * TILE, VIEW_Y + HUD_HEIGHT + level.key.y * TILE);
  }
  drawDoor(
    VIEW_X + level.door.x * TILE,
    VIEW_Y + HUD_HEIGHT + level.door.y * TILE,
    tape[MEM.DOOR_OPEN] !== 0,
  );

  const enemyX = tape[MEM.ENEMY_X] || level.enemy.x;
  const enemyY = tape[MEM.ENEMY_Y] || level.enemy.y;
  const enemyDraw = smoothEnemy(enemyX, enemyY, tape[MEM.ENEMY_DIR], tape[MEM.ENEMY_TIMER]);
  drawEnemy(VIEW_X + enemyDraw.x, VIEW_Y + HUD_HEIGHT + enemyDraw.y);

  const drawX =
    (tape[MEM.PLAYER_X] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_X]) * RENDER_SCALE;
  const drawY =
    VIEW_Y +
    HUD_HEIGHT +
    (tape[MEM.PLAYER_Y] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_Y]) * RENDER_SCALE;
  drawDave(VIEW_X + drawX, drawY, {
    dead: tape[MEM.GAME_DEAD] !== 0,
    won: tape[MEM.GAME_WIN] !== 0,
    facingRight: tape[MEM.PLAYER_FACING] !== 0,
    jumpPhase: tape[MEM.PLAYER_JUMP_PHASE],
    walkFrame: Math.floor(tape[MEM.PLAYER_SUB_X] / 4) & 1,
  });

  if (status) {
    status.textContent =
      tape[MEM.GAME_WIN] !== 0
        ? "WIN - press R"
        : tape[MEM.GAME_DEAD] !== 0
          ? "DEAD - press R"
          : "";
  }
}

function drawHud(tape) {
  ctx.fillStyle = "#030303";
  ctx.fillRect(VIEW_X, VIEW_Y, SCREEN_WIDTH, HUD_HEIGHT);
  ctx.fillStyle = "#d8d8d8";
  ctx.fillRect(VIEW_X, VIEW_Y + HUD_HEIGHT - 5, SCREEN_WIDTH, 2);
  ctx.fillStyle = "#505050";
  ctx.fillRect(VIEW_X, VIEW_Y + HUD_HEIGHT - 3, SCREEN_WIDTH, 3);
  ctx.fillStyle = "#f4f4f4";
  ctx.fillRect(VIEW_X, VIEW_Y + HUD_HEIGHT - 3, SCREEN_WIDTH, 1);

  const door = tape[MEM.DOOR_OPEN] !== 0 ? "OPEN" : "LOCK";
  const key = tape[MEM.KEY_COLLECTED] !== 0 ? "YES" : "NO";
  const mode =
    tape[MEM.GAME_WIN] !== 0 ? "WIN" : tape[MEM.GAME_DEAD] !== 0 ? "DEAD" : "READY";

  drawPixelText("ROM BF", VIEW_X + 8, VIEW_Y + 6, 2, "#45f35e", "#104018");
  drawPixelText(`SCORE ${scoreText(tape[MEM.SCORE])}`, VIEW_X + 126, VIEW_Y + 6, 2, "#dfffe4", "#23552a");
  drawPixelText(`KEY ${key}`, VIEW_X + 320, VIEW_Y + 6, 2, "#55dfff", "#124451");
  drawPixelText(`DOOR ${door}`, VIEW_X + 418, VIEW_Y + 6, 2, "#ffe35a", "#4f3e0b");
  drawPixelText(mode, VIEW_X + SCREEN_WIDTH - 70, VIEW_Y + 6, 2, "#ff7777", "#4b0b0b");
}

function scoreText(score) {
  return String(score).padStart(4, "0");
}

function drawPixelText(text, x, y, scale, color, shadow) {
  let cursor = x;
  for (const char of text.toUpperCase()) {
    const glyph = FONT[char] ?? FONT[" "];
    drawGlyph(glyph, cursor + scale, y + scale, scale, shadow);
    drawGlyph(glyph, cursor, y, scale, color);
    cursor += 6 * scale;
  }
}

function drawGlyph(glyph, x, y, scale, color) {
  ctx.fillStyle = color;
  for (let row = 0; row < glyph.length; row += 1) {
    for (let col = 0; col < glyph[row].length; col += 1) {
      if (glyph[row][col] === "1") {
        ctx.fillRect(x + col * scale, y + row * scale, scale, scale);
      }
    }
  }
}

function drawInnerShadows() {
  for (let y = 0; y < level.height; y += 1) {
    for (let x = 0; x < level.width; x += 1) {
      if (isSolid(x, y)) continue;
      const px = VIEW_X + x * TILE;
      const py = VIEW_Y + HUD_HEIGHT + y * TILE;
      if (isSolid(x, y - 1)) {
        ctx.fillStyle = "rgba(95, 0, 0, 0.55)";
        ctx.fillRect(px, py, TILE, 4);
      }
      if (isSolid(x - 1, y)) {
        ctx.fillStyle = "rgba(95, 0, 0, 0.35)";
        ctx.fillRect(px, py, 4, TILE);
      }
      if (isSolid(x + 1, y)) {
        ctx.fillStyle = "rgba(0, 0, 0, 0.35)";
        ctx.fillRect(px + TILE - 4, py, 4, TILE);
      }
    }
  }
}

function drawBrickTile(tileX, tileY) {
  const x = tileX * TILE;
  const y = VIEW_Y + HUD_HEIGHT + tileY * TILE;
  const drawX = VIEW_X + x;
  const variant = (tileX * 7 + tileY * 11) % 4;
  const base = ["#d80d08", "#e0140d", "#cf0b07", "#e21a0c"][variant];

  ctx.fillStyle = base;
  ctx.fillRect(drawX, y, TILE, TILE);
  ctx.fillStyle = "#8a0705";
  ctx.fillRect(drawX, y + 7, TILE, 2);
  ctx.fillRect(drawX, y + 15, TILE, 2);
  ctx.fillRect(drawX, y + 23, TILE, 2);
  ctx.fillRect(drawX + 15, y, 2, 8);
  ctx.fillRect(drawX + 7, y + 8, 2, 8);
  ctx.fillRect(drawX + 23, y + 16, 2, 8);

  ctx.fillStyle = "#ff3324";
  ctx.fillRect(drawX + 2, y + 2, TILE - 5, 2);
  ctx.fillStyle = "#ad0906";
  ctx.fillRect(drawX + 3, y + TILE - 4, TILE - 6, 2);

  if (!isSolid(tileX, tileY - 1)) {
    ctx.fillStyle = "#ff5a48";
    ctx.fillRect(drawX, y, TILE, 3);
  }
  if (!isSolid(tileX, tileY + 1)) {
    ctx.fillStyle = "#680302";
    ctx.fillRect(drawX, y + TILE - 4, TILE, 4);
  }
  if (!isSolid(tileX - 1, tileY)) {
    ctx.fillStyle = "#ff2418";
    ctx.fillRect(drawX, y, 3, TILE);
  }
  if (!isSolid(tileX + 1, tileY)) {
    ctx.fillStyle = "#6f0403";
    ctx.fillRect(drawX + TILE - 3, y, 3, TILE);
  }
}

function drawKey(x, y) {
  ctx.fillStyle = "#fff38f";
  ctx.fillRect(x + 10, y + 8, 12, 4);
  ctx.fillStyle = "#ffd21f";
  ctx.fillRect(x + 8, y + 12, 16, 4);
  ctx.fillRect(x + 12, y + 16, 8, 8);
  ctx.fillStyle = "#b9890b";
  ctx.fillRect(x + 14, y + 24, 4, 5);
  ctx.fillRect(x + 11, y + 29, 10, 3);
  ctx.fillStyle = "#55f7ff";
  ctx.fillRect(x + 15, y + 6, 2, 7);
  ctx.fillRect(x + 13, y + 9, 6, 2);
}

function drawCoin(x, y) {
  ctx.fillStyle = "#fff07a";
  ctx.fillRect(x + 11, y + 10, 10, 2);
  ctx.fillRect(x + 8, y + 12, 16, 4);
  ctx.fillRect(x + 7, y + 16, 18, 8);
  ctx.fillRect(x + 9, y + 24, 14, 4);
  ctx.fillStyle = "#d69a13";
  ctx.fillRect(x + 9, y + 20, 14, 3);
  ctx.fillRect(x + 12, y + 27, 9, 2);
  ctx.fillStyle = "#fff7bd";
  ctx.fillRect(x + 10, y + 13, 4, 4);
  ctx.fillStyle = "#8a5b08";
  ctx.fillRect(x + 18, y + 16, 3, 9);
}

function drawDoor(x, y, open) {
  ctx.fillStyle = "#4f260f";
  ctx.fillRect(x + 4, y, 23, 32);
  ctx.fillStyle = open ? "#3e3e3e" : "#d98743";
  ctx.fillRect(x + 6, y + 2, 19, 30);
  if (open) {
    ctx.fillStyle = "#101010";
    ctx.fillRect(x + 12, y + 4, 10, 26);
    ctx.fillStyle = "#cfcfcf";
    ctx.fillRect(x + 6, y + 2, 5, 30);
    ctx.fillRect(x + 8, y + 4, 3, 26);
    return;
  }
  ctx.fillStyle = "#a85a28";
  ctx.fillRect(x + 8, y + 6, 15, 3);
  ctx.fillRect(x + 8, y + 16, 15, 3);
  ctx.fillRect(x + 8, y + 26, 15, 3);
  ctx.fillStyle = "#f0b06a";
  ctx.fillRect(x + 8, y + 4, 14, 2);
  ctx.fillStyle = "#68401f";
  ctx.fillRect(x + 11, y + 10, 3, 5);
  ctx.fillRect(x + 18, y + 20, 3, 5);
  ctx.fillStyle = "#ffe35a";
  ctx.fillRect(x + 21, y + 17, 3, 3);
}

function drawEnemy(x, y) {
  ctx.fillStyle = "#6d0528";
  ctx.fillRect(x + 6, y + 12, 20, 20);
  ctx.fillStyle = "#ff2fb3";
  ctx.fillRect(x + 8, y + 13, 16, 14);
  ctx.fillStyle = "#9a0b72";
  ctx.fillRect(x + 8, y + 27, 5, 5);
  ctx.fillRect(x + 19, y + 27, 5, 5);
  ctx.fillStyle = "#fff2ff";
  ctx.fillRect(x + 11, y + 17, 4, 4);
  ctx.fillRect(x + 18, y + 17, 4, 4);
  ctx.fillStyle = "#1d0b1f";
  ctx.fillRect(x + 12, y + 18, 2, 2);
  ctx.fillRect(x + 19, y + 18, 2, 2);
  ctx.fillStyle = "#ffd8fb";
  ctx.fillRect(x + 10, y + 14, 9, 2);
}

function drawDave(x, y, state) {
  const s = RENDER_SCALE;
  const px = x;
  const py = y;
  const right = state.facingRight;
  const jumping = state.jumpPhase !== 0;
  const walk = jumping || state.dead || state.won ? 0 : state.walkFrame;

  function spriteRect(col, row, width, height, color) {
    const drawCol = right ? col : 16 - col - width;
    ctx.fillStyle = color;
    ctx.fillRect(px + drawCol * s, py + row * s, width * s, height * s);
  }

  spriteRect(4, 1, 7, 2, "#f02518");
  spriteRect(10, 2, 5, 2, "#f02518");
  spriteRect(5, 3, 2, 3, "#6a2a18");
  spriteRect(7, 3, 5, 5, "#ffd2ad");
  spriteRect(11, 4, 2, 2, "#ffd2ad");
  spriteRect(10, 4, 1, 1, "#101010");
  spriteRect(7, 7, 5, 1, "#8a3d24");

  spriteRect(5, 8, 6, 4, "#f1eee0");
  spriteRect(3, 9, 3, 4, "#ffd2ad");
  spriteRect(11, 9, 4, 3, "#ffd2ad");
  spriteRect(6, 12, 6, 3, "#139de2");
  spriteRect(5, 12, 2, 2, "#f1eee0");

  if (jumping) {
    spriteRect(5, 14, 3, 1, "#086aa9");
    spriteRect(10, 14, 3, 1, "#086aa9");
    spriteRect(4, 15, 5, 1, "#111");
    spriteRect(10, 15, 5, 1, "#111");
  } else if (walk === 0) {
    spriteRect(5, 14, 3, 1, "#086aa9");
    spriteRect(10, 14, 3, 1, "#086aa9");
    spriteRect(4, 15, 5, 1, "#111");
    spriteRect(10, 15, 5, 1, "#111");
  } else {
    spriteRect(6, 14, 3, 1, "#086aa9");
    spriteRect(9, 14, 3, 1, "#086aa9");
    spriteRect(5, 15, 4, 1, "#111");
    spriteRect(9, 15, 4, 1, "#111");
  }

  if (state.dead) {
    spriteRect(8, 5, 1, 1, "#101010");
    spriteRect(10, 5, 1, 1, "#101010");
    spriteRect(8, 7, 4, 1, "#c51f1f");
  }
}

function smoothEnemy(tileX, tileY, direction, timer) {
  const targetX = tileX * TILE;
  const targetY = tileY * TILE;
  if (!visualEnemy.initialized) {
    visualEnemy.initialized = true;
    visualEnemy.x = targetX;
    visualEnemy.y = targetY;
  }

  const delay = Math.max(1, level.enemy.tickDelay ?? 18);
  const clampedTimer = Math.max(0, Math.min(delay, timer));
  const progress = (delay - clampedTimer) / delay;
  let startTileX = tileX;

  if (direction !== 0 && tileX > level.enemy.minX) {
    startTileX = tileX - 1;
  } else if (direction === 0 && tileX < level.enemy.maxX) {
    startTileX = tileX + 1;
  }

  const startX = startTileX * TILE;
  visualEnemy.x = startX + (targetX - startX) * progress;
  visualEnemy.y = targetY;

  return {
    x: Math.round(visualEnemy.x),
    y: Math.round(visualEnemy.y),
  };
}

function isSolid(x, y) {
  return solidTiles.has(tileKey(x, y));
}

function tileKey(x, y) {
  return `${x},${y}`;
}

window.addEventListener("resize", resize);

window.addEventListener("keydown", (event) => {
  if (
    event.code === "ArrowLeft" ||
    event.code === "ArrowRight" ||
    event.code === "ArrowUp" ||
    event.code === "Space" ||
    event.code === "KeyR"
  ) {
    keys.add(event.code);
    if (event.code === "ArrowLeft" || event.code === "ArrowRight") {
      horizontalIntent = event.code;
    }
    event.preventDefault();
  }
});

window.addEventListener("keyup", (event) => {
  keys.delete(event.code);
  if (event.code === horizontalIntent) {
    if (keys.has("ArrowLeft")) {
      horizontalIntent = "ArrowLeft";
    } else if (keys.has("ArrowRight")) {
      horizontalIntent = "ArrowRight";
    } else {
      horizontalIntent = null;
    }
  }
});

async function main() {
  const rom = await fetch("/rom/dave.bf").then((response) => {
    if (!response.ok) throw new Error(`Failed to load ROM: ${response.status}`);
    return response.text();
  });

  const vm = new BrainfuckVM(rom, { tapeSize: 512 });
  vm.tape[MEM.PLAYER_X] = level.playerStart.x;
  vm.tape[MEM.PLAYER_Y] = level.playerStart.y;
  vm.tape[MEM.PLAYER_FACING] = 1;
  vm.tape[MEM.ENEMY_X] = level.enemy.x;
  vm.tape[MEM.ENEMY_Y] = level.enemy.y;
  vm.tape[MEM.ENEMY_DIR] = 1;

  function tickBrainfuck() {
    vm.tape[MEM.INPUT_RIGHT] = horizontalIntent === "ArrowRight" ? 1 : 0;
    vm.tape[MEM.INPUT_LEFT] = horizontalIntent === "ArrowLeft" ? 1 : 0;
    vm.tape[MEM.INPUT_JUMP] = keys.has("ArrowUp") || keys.has("Space") ? 1 : 0;
    vm.tape[MEM.INPUT_RESTART] = keys.has("KeyR") ? 1 : 0;
    vm.tape[MEM.TICK_DONE] = 0;
    vm.tape[MEM.TICK_REQUESTED] = 1;
    vm.rewind();
    return vm.runUntil((machine) => machine.tape[MEM.TICK_DONE] === 1, 600000);
  }

  let accumulator = 0;
  let previousTime = performance.now();

  function frame(now) {
    const elapsed = Math.min(now - previousTime, MAX_FRAME_MS);
    previousTime = now;
    accumulator += elapsed;

    while (accumulator >= STEP_MS) {
      tickBrainfuck();
      accumulator -= STEP_MS;
    }

    draw(vm.tape);
    requestAnimationFrame(frame);
  }

  resize();
  draw(vm.tape);
  requestAnimationFrame(frame);
}

main().catch((error) => {
  if (status) status.textContent = error.message;
});
