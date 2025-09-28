import { BrainfuckVM, MEM } from "./vm.js";
import { level } from "/game/generated/level.js";

const TILE = 32;
const LOGICAL_TILE = 16;
const RENDER_SCALE = TILE / LOGICAL_TILE;
const HUD_HEIGHT = 32;
const VISIBLE_TILES = 19;
const WORLD_WIDTH = level.width * TILE;
const SCREEN_WIDTH = Math.min(level.width, VISIBLE_TILES) * TILE;
const SCREEN_HEIGHT = HUD_HEIGHT + level.height * TILE;
const STAGE_WIDTH = 640;
const STAGE_HEIGHT = 360;
const VIEW_X = Math.floor((STAGE_WIDTH - SCREEN_WIDTH) / 2);
const VIEW_Y = Math.floor((STAGE_HEIGHT - SCREEN_HEIGHT) / 2);
const STEP_MS = 1000 / 60;
const MAX_FRAME_MS = 100;
const AUDIO_VOLUME = 2.7;

const canvas = document.querySelector("#screen");
const status = document.querySelector("#status");
const ctx = canvas.getContext("2d");
ctx.imageSmoothingEnabled = false;

const keys = new Set();
const solidTiles = new Set(level.solids.map((solid) => tileKey(solid.x, solid.y)));
const platformTiles = new Set((level.platforms ?? []).map((solid) => tileKey(solid.x, solid.y)));
const visualEnemy = { initialized: false, x: 0, y: 0 };
let screenMode = "title";
let startRequested = false;
let presentedLevel = 1;
let transitionUntil = 0;
let transitionFromLevel = 1;
let jumpQueued = false;
let jumpHeld = false;
let horizontalIntent = null;
let audioContext = null;
let lastAudioSeq = 0;

const FONT_CHARS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ:-/' ";
const FONT_BITS =
  "01110,10001,10011,10101,11001,10001,01110;" +
  "00100,01100,00100,00100,00100,00100,01110;" +
  "01110,10001,00001,00010,00100,01000,11111;" +
  "11110,00001,00001,01110,00001,00001,11110;" +
  "00010,00110,01010,10010,11111,00010,00010;" +
  "11111,10000,10000,11110,00001,00001,11110;" +
  "00110,01000,10000,11110,10001,10001,01110;" +
  "11111,00001,00010,00100,01000,01000,01000;" +
  "01110,10001,10001,01110,10001,10001,01110;" +
  "01110,10001,10001,01111,00001,00010,11100;" +
  "01110,10001,10001,11111,10001,10001,10001;" +
  "11110,10001,10001,11110,10001,10001,11110;" +
  "01111,10000,10000,10000,10000,10000,01111;" +
  "11110,10001,10001,10001,10001,10001,11110;" +
  "11111,10000,10000,11110,10000,10000,11111;" +
  "11111,10000,10000,11110,10000,10000,10000;" +
  "01111,10000,10000,10011,10001,10001,01111;" +
  "10001,10001,10001,11111,10001,10001,10001;" +
  "01110,00100,00100,00100,00100,00100,01110;" +
  "00111,00010,00010,00010,00010,10010,01100;" +
  "10001,10010,10100,11000,10100,10010,10001;" +
  "10000,10000,10000,10000,10000,10000,11111;" +
  "10001,11011,10101,10101,10001,10001,10001;" +
  "10001,11001,10101,10011,10001,10001,10001;" +
  "01110,10001,10001,10001,10001,10001,01110;" +
  "11110,10001,10001,11110,10000,10000,10000;" +
  "01110,10001,10001,10001,10101,10010,01101;" +
  "11110,10001,10001,11110,10100,10010,10001;" +
  "01111,10000,10000,01110,00001,00001,11110;" +
  "11111,00100,00100,00100,00100,00100,00100;" +
  "10001,10001,10001,10001,10001,10001,01110;" +
  "10001,10001,10001,10001,10001,01010,00100;" +
  "10001,10001,10001,10101,10101,10101,01010;" +
  "10001,10001,01010,00100,01010,10001,10001;" +
  "10001,10001,01010,00100,00100,00100,00100;" +
  "11111,00001,00010,00100,01000,10000,11111;" +
  "00000,00100,00100,00000,00100,00100,00000;" +
  "00000,00000,00000,11111,00000,00000,00000;" +
  "00001,00010,00010,00100,01000,01000,10000;" +
  "00100,00100,00000,00000,00000,00000,00000;" +
  "00000,00000,00000,00000,00000,00000,00000";

const FONT = Object.freeze(
  Object.fromEntries(FONT_BITS.split(";").map((glyph, index) => [
    FONT_CHARS[index],
    glyph.split(","),
  ])),
);


function resize() {
  canvas.width = STAGE_WIDTH;
  canvas.height = STAGE_HEIGHT;
  ctx.imageSmoothingEnabled = false;
}

function drawTitleScreen(now) {
  ctx.fillStyle = "#020202";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  drawTitleCave();
  drawTitleLogo(120, 34);
  drawPixelText("A BRAINFUCK GAME", 224, 102, 2, "#f6f6f6", "#303030");
  drawTitleMiniScene(now);

  if (Math.floor(now / 420) % 2 === 0) {
    drawPixelText("PRESS SPACE", 254, 318, 2, "#ffffff", "#303030");
  }
  drawPixelText("FROM JOHN ROMERO'S DANGEROUS DAVE", 220, 342, 1, "#bfbfbf", "#303030");
}

function drawTitleLogo(x, y) {
  drawPixelText("DAVEFUCK", x + 83, y + 23, 5, "#bd7141", "#2a2a2a");
  drawPixelText("DAVEFUCK", x + 80, y + 20, 5, "#e49a62", "#6e3f29");
}

function drawTitleCave() {
  const blocks = [
    [154, 178, 24, 100],
    [154, 278, 250, 18],
    [302, 278, 74, 18],
    [270, 214, 24, 64],
    [270, 214, 96, 18],
    [430, 246, 24, 50],
    [430, 278, 92, 18],
    [498, 178, 24, 100],
    [398, 190, 78, 18],
  ];
  for (const [x, y, width, height] of blocks) {
    drawRockRect(x, y, width, height);
  }
}

function drawRockRect(x, y, width, height) {
  ctx.save();
  ctx.beginPath();
  ctx.rect(x, y, width, height);
  ctx.clip();
  ctx.fillStyle = "#2a160e";
  ctx.fillRect(x, y, width, height);
  for (let py = y; py < y + height; py += 12) {
    const offset = (Math.floor((py - y) / 12) % 2) * 8;
    for (let px = x - offset; px < x + width; px += 16) {
      const brickX = px + (Math.floor((py - y) / 12) % 2) * 8;
      const clippedWidth = Math.min(16, x + width - brickX);
      const clippedHeight = Math.min(12, y + height - py);
      if (brickX < x || clippedWidth <= 0 || clippedHeight <= 0) continue;
      ctx.fillStyle = "#bd7141";
      ctx.fillRect(brickX, py, clippedWidth, clippedHeight);
      ctx.fillStyle = "#6e3f29";
      ctx.fillRect(brickX, py + 10, clippedWidth, 2);
      ctx.fillRect(brickX + 14, py, 2, clippedHeight);
      ctx.fillStyle = "#e49a62";
      ctx.fillRect(brickX + 1, py + 1, Math.min(12, clippedWidth - 1), 2);
    }
  }
  ctx.fillStyle = "#422417";
  ctx.fillRect(x, y + height - 3, width, 3);
  ctx.restore();
}

function drawTitleMiniScene(now) {
  const walkFrame = Math.floor(now / 160) & 1;
  const bob = Math.floor(now / 220) % 2;
  drawDave(190, 246 + bob, {
    dead: false,
    won: false,
    facingRight: true,
    jumpPhase: 0,
    walkFrame,
  });
  drawTitleTrophy(316, 230);
  drawTitleFlame(314, 254, now);
  drawTitleFlame(472, 254, now + 180);
}

function drawTitleTrophy(x, y) {
  ctx.fillStyle = "#7a4f0d";
  ctx.fillRect(x + 9, y + 20, 8, 4);
  ctx.fillRect(x + 6, y + 24, 14, 4);
  ctx.fillStyle = "#ffd51f";
  ctx.fillRect(x + 6, y + 6, 14, 10);
  ctx.fillRect(x + 10, y + 15, 6, 7);
  ctx.fillStyle = "#fff38f";
  ctx.fillRect(x + 9, y + 4, 8, 3);
  ctx.fillRect(x + 12, y + 2, 3, 17);
  ctx.fillStyle = "#b9890b";
  ctx.fillRect(x + 3, y + 8, 3, 6);
  ctx.fillRect(x + 20, y + 8, 3, 6);
  ctx.fillRect(x + 8, y + 17, 10, 3);
  ctx.fillStyle = "#55f7ff";
  ctx.fillRect(x + 12, y, 3, 7);
  ctx.fillRect(x + 9, y + 4, 9, 2);
}

function drawTitleFlame(x, y, now) {
  const lift = Math.floor(now / 120) % 2;
  ctx.fillStyle = "#4a0802";
  ctx.fillRect(x + 5, y + 20, 20, 4);
  ctx.fillStyle = "#f02713";
  ctx.fillRect(x + 7, y + 10 + lift, 16, 12);
  ctx.fillStyle = "#ff8a00";
  ctx.fillRect(x + 10, y + 6 - lift, 10, 16);
  ctx.fillStyle = "#ffe13d";
  ctx.fillRect(x + 13, y + 12, 5, 9);
}

function draw(tape) {
  const cameraX = cameraForTape(tape);
  const currentLevel = tape[MEM.CURRENT_LEVEL] || 1;
  ctx.fillStyle = "#080808";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  drawHud(tape);
  ctx.save();
  ctx.beginPath();
  ctx.rect(VIEW_X, VIEW_Y + HUD_HEIGHT, SCREEN_WIDTH, level.height * TILE);
  ctx.clip();
  ctx.fillStyle = "#050505";
  ctx.fillRect(VIEW_X, VIEW_Y + HUD_HEIGHT, SCREEN_WIDTH, level.height * TILE);
  drawInnerShadows(cameraX);

  for (const solid of level.solids) {
    if (isInCamera(solid.x, cameraX)) {
      if (platformTiles.has(tileKey(solid.x, solid.y))) {
        drawPlatformTile(solid.x, solid.y, cameraX);
      } else {
        drawBrickTile(solid.x, solid.y, cameraX);
      }
    }
  }

  for (const decoration of level.decorations ?? []) {
    if (!isInCamera(decoration.x, cameraX)) continue;
    const x = VIEW_X + decoration.x * TILE - cameraX;
    const y = VIEW_Y + HUD_HEIGHT + decoration.y * TILE;
    if (decoration.kind === "F") {
      drawFire(x, y, performance.now());
    } else if (decoration.kind === "V") {
      drawVine(x, y, performance.now());
    } else if (decoration.kind === "W") {
      drawWater(x, y, performance.now());
    }
  }

  level.coins.forEach((coin, index) => {
    if (tape[MEM.COIN_BASE + index] === 0) {
      drawCoin(
        VIEW_X + coin.x * TILE - cameraX,
        VIEW_Y + HUD_HEIGHT + coin.y * TILE,
        coin.kind ?? "C",
      );
    }
  });
  const currentKey = entityForLevel(level.keys, currentLevel) ?? level.key;
  if (currentKey && tape[MEM.KEY_COLLECTED] === 0) {
    drawKey(VIEW_X + currentKey.x * TILE - cameraX, VIEW_Y + HUD_HEIGHT + currentKey.y * TILE + 4);
  }
  for (const door of level.doors ?? [level.door]) {
    drawDoor(
      VIEW_X + door.x * TILE - cameraX,
      VIEW_Y + HUD_HEIGHT + door.y * TILE,
      door.level < currentLevel || (door.level === currentLevel && tape[MEM.DOOR_OPEN] !== 0),
    );
  }

  const activeEnemy = entityForLevel(level.enemies, currentLevel);
  if (activeEnemy) {
    const enemyX = tape[MEM.ENEMY_X] || activeEnemy.x;
    const enemyY = tape[MEM.ENEMY_Y] || activeEnemy.y;
    const enemyDraw = smoothEnemy(
      activeEnemy,
      enemyX,
      enemyY,
      tape[MEM.ENEMY_DIR],
      tape[MEM.ENEMY_TIMER],
    );
    drawEnemy(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
  }

  const drawX =
    (tape[MEM.PLAYER_X] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_X]) * RENDER_SCALE;
  const drawY =
    VIEW_Y +
    HUD_HEIGHT +
    (tape[MEM.PLAYER_Y] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_Y]) * RENDER_SCALE;
  drawDave(VIEW_X + drawX - cameraX, drawY, {
    dead: tape[MEM.GAME_DEAD] !== 0,
    won: tape[MEM.GAME_WIN] !== 0,
    facingRight: tape[MEM.PLAYER_FACING] !== 0,
    jumpPhase: tape[MEM.PLAYER_JUMP_PHASE],
    walkFrame: Math.floor(tape[MEM.PLAYER_SUB_X] / 4) & 1,
  });
  ctx.restore();

  if (status) {
    status.textContent =
      tape[MEM.GAME_WIN] !== 0
        ? "WIN - press R"
        : tape[MEM.GAME_DEAD] !== 0
          ? "DEAD - press R"
          : "";
  }
}

function cameraForTape(tape) {
  const section = activeLevelSection(tape[MEM.CURRENT_LEVEL] || 1);
  const playerX =
    (tape[MEM.PLAYER_X] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_X]) * RENDER_SCALE;
  const sectionStart = section.startX * TILE;
  const sectionWidth = section.width * TILE;
  const minCameraX = sectionStart;
  const maxCameraX = Math.max(sectionStart, sectionStart + sectionWidth - SCREEN_WIDTH);
  return Math.round(
    Math.max(minCameraX, Math.min(maxCameraX, playerX - SCREEN_WIDTH * 0.42)),
  );
}

function activeLevelSection(currentLevel) {
  return (
    level.sections?.find((section) => section.kind === "level" && section.level === currentLevel) ??
    { startX: 0, width: Math.min(level.width, VISIBLE_TILES) }
  );
}

function entityForLevel(entities, currentLevel) {
  return entities?.find((entity) => entity.level === currentLevel);
}

function isInCamera(tileX, cameraX) {
  const x = tileX * TILE;
  return x + TILE >= cameraX && x <= cameraX + SCREEN_WIDTH;
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
  const currentLevel = tape[MEM.CURRENT_LEVEL] || 1;
  const restartPrompt = tape[MEM.GAME_WIN] !== 0 || tape[MEM.GAME_DEAD] !== 0;

  drawPixelText("DAVE", VIEW_X + 8, VIEW_Y + 6, 2, "#45f35e", "#104018");
  drawPixelText(`SCORE ${scoreText(tape[MEM.SCORE])}`, VIEW_X + 80, VIEW_Y + 6, 2, "#dfffe4", "#23552a");
  drawPixelText(`LV ${String(currentLevel).padStart(2, "0")}`, VIEW_X + 238, VIEW_Y + 6, 2, "#45f35e", "#104018");
  if (restartPrompt) {
    drawPixelText("PRESS R TO RESTART", VIEW_X + 356, VIEW_Y + 6, 2, "#ff7777", "#4b0b0b");
  } else {
    drawPixelText(`KEY ${key}`, VIEW_X + 330, VIEW_Y + 6, 2, "#55dfff", "#124451");
    drawPixelText(`DOOR ${door}`, VIEW_X + 424, VIEW_Y + 6, 2, "#ffe35a", "#4f3e0b");
    drawPixelText("READY", VIEW_X + SCREEN_WIDTH - 70, VIEW_Y + 6, 2, "#ff7777", "#4b0b0b");
  }
}

function drawTransitionScreen(fromLevel, toLevel, now) {
  ctx.fillStyle = "#020202";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  drawHud({
    [MEM.SCORE]: 0,
    [MEM.CURRENT_LEVEL]: toLevel,
    [MEM.KEY_COLLECTED]: 0,
    [MEM.DOOR_OPEN]: 0,
    [MEM.GAME_WIN]: 0,
    [MEM.GAME_DEAD]: 0,
  });

  const corridorY = VIEW_Y + HUD_HEIGHT + 120;
  for (let x = VIEW_X; x < VIEW_X + SCREEN_WIDTH; x += TILE) {
    drawTransitionBrick(x, corridorY - TILE);
    drawTransitionBrick(x, corridorY + TILE);
  }
  drawDoor(VIEW_X - 4, corridorY, true);
  const progress = Math.min(1, Math.max(0, (1800 - (transitionUntil - now)) / 1800));
  const daveX = VIEW_X + 90 + progress * (SCREEN_WIDTH - 100);
  const remLevels = toLevel - fromLevel;
  const levelWorld = remLevels === 1 ? "LEVEL" : "LEVELS"
  drawDave(daveX - 60, corridorY + 2, {
    dead: false,
    won: false,
    facingRight: true,
    jumpPhase: 0,
    walkFrame: Math.floor(now / 300) & 1,
  });
  drawPixelText(
    `${String(remLevels).padStart(2, "0")} MORE ${levelWorld} TO GO`,
    VIEW_X + 182,
    VIEW_Y + HUD_HEIGHT + 60,
    2,
    "#ffffff",
    "#303030",
  );
  drawPixelText(
    `GOOD JOB`,
    VIEW_X + 256,
    VIEW_Y + HUD_HEIGHT + 210,
    2,
    "#55dfff",
    "#124451",
  );
}

function drawTransitionBrick(x, y) {
  ctx.fillStyle = "#1229c8";
  ctx.fillRect(x, y, TILE, TILE);
  ctx.fillStyle = "#061064";
  ctx.fillRect(x, y + 7, TILE, 2);
  ctx.fillRect(x, y + 15, TILE, 2);
  ctx.fillRect(x, y + 23, TILE, 2);
  ctx.fillRect(x + 15, y, 2, 8);
  ctx.fillRect(x + 7, y + 8, 2, 8);
  ctx.fillRect(x + 23, y + 16, 2, 8);
  ctx.fillStyle = "#295cff";
  ctx.fillRect(x + 2, y + 2, TILE - 5, 2);
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

function drawInnerShadows(cameraX) {
  for (let y = 0; y < level.height; y += 1) {
    for (let x = 0; x < level.width; x += 1) {
      if (isFullBrick(x, y)) continue;
      if (!isInCamera(x, cameraX)) continue;
      const px = VIEW_X + x * TILE - cameraX;
      const py = VIEW_Y + HUD_HEIGHT + y * TILE;
      if (isFullBrick(x, y - 1)) {
        ctx.fillStyle = "rgba(95, 0, 0, 0.55)";
        ctx.fillRect(px, py, TILE, 4);
      }
      if (isFullBrick(x - 1, y)) {
        ctx.fillStyle = "rgba(95, 0, 0, 0.35)";
        ctx.fillRect(px, py, 4, TILE);
      }
      if (isFullBrick(x + 1, y)) {
        ctx.fillStyle = "rgba(0, 0, 0, 0.35)";
        ctx.fillRect(px + TILE - 4, py, 4, TILE);
      }
    }
  }
}

function drawBrickTile(tileX, tileY, cameraX) {
  const x = tileX * TILE;
  const y = VIEW_Y + HUD_HEIGHT + tileY * TILE;
  const drawX = VIEW_X + x - cameraX;
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

function drawPlatformTile(tileX, tileY, cameraX) {
  const x = tileX * TILE;
  const y = VIEW_Y + HUD_HEIGHT + tileY * TILE;
  const drawX = VIEW_X + x - cameraX;

  ctx.fillStyle = "rgba(0, 0, 0, 0.45)";
  ctx.fillRect(drawX + TILE - 3, y + 5, 5, 21);
  ctx.fillStyle = "rgba(64, 0, 54, 0.55)";
  ctx.fillRect(drawX + 3, y + 24, TILE - 3, 4);

  ctx.fillStyle = "#9b0e91";
  ctx.fillRect(drawX, y, TILE, 18);
  ctx.fillStyle = "#ff5cff";
  ctx.fillRect(drawX, y, TILE, 3);
  ctx.fillRect(drawX + 2, y + 18, TILE - 4, 2);
  ctx.fillStyle = "#551054";
  ctx.fillRect(drawX, y + 20, TILE, 4);
  ctx.fillStyle = "#1b071c";
  ctx.fillRect(drawX + 4, y + 9, 5, 5);
  ctx.fillRect(drawX + 16, y + 9, 5, 5);
  ctx.fillRect(drawX + 28, y + 9, 4, 5);
  ctx.fillStyle = "#ff9cff";
  ctx.fillRect(drawX + 2, y + 3, TILE - 4, 2);
  ctx.fillStyle = "#ff2fe8";
  for (let offset = 0; offset < TILE; offset += 6) {
    ctx.fillRect(drawX + offset, y + 2, 3, 2);
    ctx.fillRect(drawX + offset + 2, y + 22, 3, 2);
  }
}

function drawKey(x, y) {
  ctx.fillStyle = "#7a4f0d";
  ctx.fillRect(x + 9, y + 20, 8, 4);
  ctx.fillRect(x + 6, y + 24, 14, 4);
  ctx.fillStyle = "#ffd51f";
  ctx.fillRect(x + 6, y + 6, 14, 10);
  ctx.fillRect(x + 10, y + 15, 6, 7);
  ctx.fillStyle = "#fff38f";
  ctx.fillRect(x + 9, y + 4, 8, 3);
  ctx.fillRect(x + 12, y + 2, 3, 17);
  ctx.fillStyle = "#b9890b";
  ctx.fillRect(x + 3, y + 8, 3, 6);
  ctx.fillRect(x + 20, y + 8, 3, 6);
  ctx.fillRect(x + 8, y + 17, 10, 3);
  ctx.fillStyle = "#55f7ff";
  ctx.fillRect(x + 12, y, 3, 7);
  ctx.fillRect(x + 9, y + 4, 9, 2);
}

function drawCoin(x, y, kind = "C") {
  if (kind === "R") {
    drawDiamond(x, y, "#ff382b", "#ff887d", "#7a0707");
    return;
  }
  if (kind === "B") {
    drawDiamond(x, y, "#30dfff", "#a7ffff", "#086f88");
    return;
  }
  drawPurpleCoin(x, y);
}

function drawPurpleCoin(x, y) {
  ctx.fillStyle = "#ff72ff";
  ctx.fillRect(x + 11, y + 8, 10, 2);
  ctx.fillRect(x + 8, y + 10, 16, 4);
  ctx.fillRect(x + 7, y + 14, 18, 10);
  ctx.fillRect(x + 10, y + 24, 12, 4);
  ctx.fillStyle = "#ad20bd";
  ctx.fillRect(x + 9, y + 20, 14, 3);
  ctx.fillRect(x + 12, y + 27, 8, 2);
  ctx.fillStyle = "#ffcaff";
  ctx.fillRect(x + 11, y + 12, 4, 4);
  ctx.fillStyle = "#661077";
  ctx.fillRect(x + 18, y + 14, 4, 10);
}

function drawDiamond(x, y, base, shine, shadow) {
  ctx.fillStyle = shadow;
  ctx.fillRect(x + 11, y + 25, 12, 3);
  ctx.fillStyle = base;
  ctx.fillRect(x + 8, y + 10, 16, 5);
  ctx.fillRect(x + 6, y + 15, 20, 5);
  ctx.fillRect(x + 9, y + 20, 14, 4);
  ctx.fillRect(x + 12, y + 24, 8, 3);
  ctx.fillStyle = shine;
  ctx.fillRect(x + 10, y + 11, 7, 3);
  ctx.fillRect(x + 8, y + 15, 4, 3);
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(x + 12, y + 12, 3, 2);
  ctx.fillStyle = shadow;
  ctx.fillRect(x + 20, y + 15, 4, 6);
  ctx.fillRect(x + 16, y + 22, 4, 3);
}

function drawFire(x, y, now) {
  const lift = Math.floor(now / 110) % 2;
  ctx.fillStyle = "#4a0802";
  ctx.fillRect(x + 6, y + 24, 20, 4);
  ctx.fillStyle = "#f02713";
  ctx.fillRect(x + 7, y + 13 + lift, 18, 12);
  ctx.fillStyle = "#ff8a00";
  ctx.fillRect(x + 10, y + 8 - lift, 12, 17);
  ctx.fillStyle = "#ffe13d";
  ctx.fillRect(x + 14, y + 14, 5, 10);
  ctx.fillStyle = "#ffcab0";
  ctx.fillRect(x + 17, y + 20, 3, 4);
}

function drawVine(x, y, now) {
  const phase = Math.floor(now / 140) % 4;
  const colors = ["#ff47f6", "#ff8cff", "#9e1bbb"];
  for (let strand = 0; strand < 3; strand += 1) {
    ctx.fillStyle = colors[strand % colors.length];
    const baseX = x + 8 + strand * 7;
    for (let py = 2; py < TILE - 2; py += 4) {
      const wave = ((py / 4 + phase + strand) % 3) - 1;
      ctx.fillRect(baseX + wave * 2, y + py, 3, 4);
    }
  }
}

function drawWater(x, y, now) {
  const wave = Math.floor(now / 180) % 4;
  ctx.fillStyle = "#1727d9";
  ctx.fillRect(x, y + 10, TILE, 22);
  ctx.fillStyle = "#385dff";
  ctx.fillRect(x, y + 8, TILE, 4);
  ctx.fillStyle = "#79aaff";
  for (let offset = -wave * 4; offset < TILE; offset += 12) {
    ctx.fillRect(x + offset, y + 13, 4, 3);
    ctx.fillRect(x + offset + 7, y + 24, 3, 2);
  }
  ctx.fillStyle = "#0a1680";
  ctx.fillRect(x, y + 30, TILE, 2);
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
  const px = x + s;
  const py = y + s;
  const right = state.facingRight;
  const jumping = state.jumpPhase !== 0;
  const walk = jumping || state.dead || state.won ? 0 : state.walkFrame;
  const skin = "#ffd2ad";
  const skinDark = "#d58b67";
  const cap = "#f02518";
  const hair = "#6a2a18";
  const shirt = "#f1eee0";
  const pants = "#139de2";
  const pantsDark = "#086aa9";
  const shoe = "#101010";

  function spriteRect(col, row, width, height, color) {
    const drawCol = (right ? col : 16 - col - width) - 1;
    ctx.fillStyle = color;
    ctx.fillRect(px + drawCol * s, py + (row - 1) * s, width * s, height * s);
  }

  spriteRect(4, 1, 7, 2, cap);
  spriteRect(3, 2, 12, 2, cap);
  spriteRect(5, 1, 3, 1, "#ff5b4a");
  spriteRect(4, 3, 3, 4, hair);
  spriteRect(7, 4, 5, 4, skin);
  spriteRect(11, 5, 2, 2, skin);
  spriteRect(10, 5, 1, 1, shoe);
  spriteRect(7, 7, 5, 1, "#8a3d24");
  spriteRect(6, 8, 6, 1, "#b82920");

  spriteRect(5, 9, 7, 3, shirt);
  spriteRect(6, 12, 6, 1, shirt);
  if (walk === 0) {
    spriteRect(3, 9, 3, 4, skin);
    spriteRect(4, 12, 2, 1, skinDark);
  } else {
    spriteRect(2, 10, 4, 3, skin);
    spriteRect(2, 12, 2, 1, skinDark);
  }
  if (walk === 0) {
    spriteRect(11, 9, 4, 3, skin);
    spriteRect(13, 11, 2, 1, skinDark);
  } else {
    spriteRect(11, 8, 3, 4, skin);
    spriteRect(12, 12, 2, 1, skinDark);
  }

  if (jumping) {
    spriteRect(5, 12, 7, 2, pants);
    spriteRect(6, 13, 3, 1, "#55c4ff");
    spriteRect(4, 14, 4, 1, pantsDark);
    spriteRect(10, 14, 4, 1, pantsDark);
    spriteRect(3, 15, 5, 1, shoe);
    spriteRect(10, 15, 5, 1, shoe);
  } else if (walk === 0) {
    spriteRect(5, 12, 7, 3, pants);
    spriteRect(5, 13, 3, 1, "#55c4ff");
    spriteRect(4, 15, 5, 1, shoe);
    spriteRect(10, 15, 4, 1, shoe);
  } else {
    spriteRect(5, 12, 7, 3, pants);
    spriteRect(6, 13, 3, 1, "#55c4ff");
    spriteRect(3, 15, 5, 1, shoe);
    spriteRect(9, 15, 5, 1, shoe);
  }

  if (state.dead) {
    spriteRect(5, 0, 10, 1, "#7a0505");
    spriteRect(6, 4, 2, 1, shoe);
    spriteRect(6, 5, 1, 1, shoe);
    spriteRect(10, 4, 2, 1, shoe);
    spriteRect(11, 5, 1, 1, shoe);
    spriteRect(7, 6, 5, 1, "#c51f1f");
    spriteRect(4, 8, 9, 1, "#c51f1f");
    spriteRect(5, 9, 7, 1, "#6d0505");
    spriteRect(4, 14, 10, 2, shoe);
  }
}

function smoothEnemy(enemy, tileX, tileY, direction, timer) {
  const startX = tileX * TILE;
  const targetY = tileY * TILE;
  if (!visualEnemy.initialized) {
    visualEnemy.initialized = true;
    visualEnemy.x = startX;
    visualEnemy.y = targetY;
  }

  const delay = Math.max(1, enemy.tickDelay ?? 18);
  const clampedTimer = Math.max(0, Math.min(delay, timer));
  const progress = (delay - clampedTimer) / delay;
  let targetTileX = tileX;

  if (direction !== 0 && tileX < enemy.maxX) {
    targetTileX = tileX + 1;
  } else if (direction === 0 && tileX > enemy.minX) {
    targetTileX = tileX - 1;
  }

  const targetX = targetTileX * TILE;
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

function isFullBrick(x, y) {
  const key = tileKey(x, y);
  return solidTiles.has(key) && !platformTiles.has(key);
}

function tileKey(x, y) {
  return `${x},${y}`;
}

function ensureAudio() {
  const AudioContext = window.AudioContext || window.webkitAudioContext;
  if (!AudioContext) return null;
  if (!audioContext) audioContext = new AudioContext();
  if (audioContext.state === "suspended") audioContext.resume();
  return audioContext;
}

function consumeAudio(tape) {
  const seq = tape[MEM.AUDIO_SEQ];
  if (seq === lastAudioSeq) return;
  lastAudioSeq = seq;
  const event = tape[MEM.AUDIO_EVENT];
  if (event === 0) return;
  playAudioEvent(event);
}

function playAudioEvent(event) {
  const audio = ensureAudio();
  if (!audio) return;

  if (event === 1) {
    playJumpSound();
  } else if (event === 2) {
    playLandSound();
  } else if (event === 3) {
    playPickupSound();
  } else if (event === 4) {
    playDoorSound();
  } else if (event === 5) {
    playDeathSound();
  } else if (event === 6) {
    playWinSound();
  }
}

function playStartJingle() {
  const audio = ensureAudio();
  if (!audio) return;
  arpeggio([196, 262, 330, 392, 523], 0.055, "square", 0.035);
  chord([392, 494, 659], 0.18, "triangle", 0.032, 0.28);
  noise(0.055, 0.018, 0.02, "highpass", 1200);
}

function playJumpSound() {
  tone(240, 0.045, "square", 0.035, 150);
  tone(510, 0.11, "square", 0.052, 310, 0.018);
  tone(940, 0.055, "triangle", 0.025, -160, 0.07);
  noise(0.028, 0.015, 0, "highpass", 1300);
}

function playLandSound() {
  tone(145, 0.055, "triangle", 0.06, -65);
  tone(86, 0.075, "square", 0.032, -20, 0.01);
  noise(0.06, 0.045, 0, "lowpass", 900);
}

function playPickupSound() {
  arpeggio([784, 1175, 1568, 2093], 0.026, "square", 0.04);
  tone(2637, 0.05, "triangle", 0.028, -220, 0.09);
  noise(0.045, 0.018, 0.012, "highpass", 2800);
}

function playDoorSound() {
  noise(0.035, 0.04, 0, "bandpass", 900);
  tone(220, 0.055, "square", 0.04, -55, 0.01);
  arpeggio([330, 440, 554, 740], 0.045, "triangle", 0.045);
  chord([370, 554, 880], 0.11, "square", 0.025, 0.19);
}

function playDeathSound() {
  tone(260, 0.24, "sawtooth", 0.055, -180);
  tone(130, 0.18, "square", 0.038, -60, 0.035);
  noise(0.18, 0.065, 0.015, "lowpass", 1200);
  arpeggio([220, 196, 165, 110], 0.055, "square", 0.032);
}

function playWinSound() {
  arpeggio([523, 659, 784, 1047, 1319], 0.06, "square", 0.046);
  chord([523, 659, 784], 0.22, "triangle", 0.032, 0.28);
  tone(1568, 0.12, "square", 0.028, 130, 0.34);
}

function tone(frequency, duration, type, volume, slide = 0, delay = 0) {
  const audio = audioContext;
  if (!audio) return;
  const start = audio.currentTime + delay;
  const outputVolume = Math.min(volume * AUDIO_VOLUME, 0.18);
  const osc = audio.createOscillator();
  const gain = audio.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(frequency, start);
  if (slide !== 0) {
    osc.frequency.linearRampToValueAtTime(Math.max(20, frequency + slide), start + duration);
  }
  gain.gain.setValueAtTime(0.0001, start);
  gain.gain.exponentialRampToValueAtTime(outputVolume, start + 0.006);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
  osc.connect(gain);
  gain.connect(audio.destination);
  osc.start(start);
  osc.stop(start + duration + 0.02);
}

function arpeggio(notes, step, type, volume) {
  notes.forEach((note, index) => tone(note, step * 1.4, type, volume, 0, index * step));
}

function chord(notes, duration, type, volume, delay = 0) {
  const eachVolume = volume / Math.max(1, Math.sqrt(notes.length));
  notes.forEach((note) => tone(note, duration, type, eachVolume, 0, delay));
}

function noise(duration, volume, delay = 0, filterType = "highpass", filterFrequency = 1000) {
  const audio = audioContext;
  if (!audio) return;
  const outputVolume = Math.min(volume * AUDIO_VOLUME, 0.16);
  const start = audio.currentTime + delay;
  const length = Math.max(1, Math.floor(audio.sampleRate * duration));
  const buffer = audio.createBuffer(1, length, audio.sampleRate);
  const data = buffer.getChannelData(0);
  for (let index = 0; index < length; index += 1) {
    data[index] = Math.random() * 2 - 1;
  }
  const source = audio.createBufferSource();
  const filter = audio.createBiquadFilter();
  const gain = audio.createGain();
  source.buffer = buffer;
  filter.type = filterType;
  filter.frequency.setValueAtTime(filterFrequency, start);
  gain.gain.setValueAtTime(outputVolume, start);
  gain.gain.exponentialRampToValueAtTime(0.0001, start + duration);
  source.connect(filter);
  filter.connect(gain);
  gain.connect(audio.destination);
  source.start(start);
}

window.addEventListener("resize", resize);

window.addEventListener("keydown", (event) => {
  if (
    event.code === "ArrowLeft" ||
    event.code === "ArrowRight" ||
    event.code === "ArrowUp" ||
    event.code === "Space" ||
    event.code === "Enter" ||
    event.code === "KeyR"
  ) {
    ensureAudio();
    if (screenMode === "title" && (event.code === "Space" || event.code === "Enter")) {
      startRequested = true;
      event.preventDefault();
      return;
    }

    keys.add(event.code);
    if ((event.code === "ArrowUp" || event.code === "Space") && !jumpHeld) {
      jumpQueued = true;
      jumpHeld = true;
    }
    if (event.code === "ArrowLeft" || event.code === "ArrowRight") {
      horizontalIntent = event.code;
    }
    event.preventDefault();
  }
});

window.addEventListener("pointerdown", () => {
  ensureAudio();
});

window.addEventListener("keyup", (event) => {
  keys.delete(event.code);
  if (event.code === "ArrowUp" || event.code === "Space") {
    jumpHeld = keys.has("ArrowUp") || keys.has("Space");
  }
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
  vm.tape[MEM.ENEMY_X] = level.enemy?.x ?? 0;
  vm.tape[MEM.ENEMY_Y] = level.enemy?.y ?? 0;
  vm.tape[MEM.ENEMY_DIR] = 1;

  function tickBrainfuck() {
    const jumpInput = jumpQueued ? 1 : 0;
    jumpQueued = false;
    vm.tape[MEM.INPUT_RIGHT] = horizontalIntent === "ArrowRight" ? 1 : 0;
    vm.tape[MEM.INPUT_LEFT] = horizontalIntent === "ArrowLeft" ? 1 : 0;
    vm.tape[MEM.INPUT_JUMP] = jumpInput;
    vm.tape[MEM.INPUT_RESTART] = keys.has("KeyR") ? 1 : 0;
    vm.tape[MEM.TICK_DONE] = 0;
    vm.tape[MEM.TICK_REQUESTED] = 1;
    vm.rewind();
    return vm.runUntil((machine) => machine.tape[MEM.TICK_DONE] === 1, 900000);
  }

  let accumulator = 0;
  let previousTime = performance.now();

  function frame(now) {
    if (screenMode === "title") {
      if (startRequested) {
        startRequested = false;
        screenMode = "game";
        accumulator = 0;
        previousTime = now;
        playStartJingle();
      } else {
        drawTitleScreen(now);
        requestAnimationFrame(frame);
        return;
      }
    }

    const elapsed = Math.min(now - previousTime, MAX_FRAME_MS);
    previousTime = now;
    accumulator += elapsed;

    while (accumulator >= STEP_MS) {
      const previousLevel = vm.tape[MEM.CURRENT_LEVEL] || 1;
      tickBrainfuck();
      consumeAudio(vm.tape);
      const currentLevel = vm.tape[MEM.CURRENT_LEVEL] || 1;
      if (currentLevel !== previousLevel) {
        transitionFromLevel = previousLevel;
        presentedLevel = currentLevel;
        transitionUntil = now + 2000;
      }
      accumulator -= STEP_MS;
    }

    if (transitionUntil > now && presentedLevel > transitionFromLevel) {
      drawTransitionScreen(transitionFromLevel, presentedLevel, now);
    } else {
      draw(vm.tape);
    }
    requestAnimationFrame(frame);
  }

  resize();
  drawTitleScreen(performance.now());
  requestAnimationFrame(frame);
}

main().catch((error) => {
  if (status) status.textContent = error.message;
});
