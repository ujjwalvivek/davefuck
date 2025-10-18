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
const JETPACK_MAX_FUEL = 150;
const JETPACK_BASE = MEM.COIN_BASE + (level.coins?.length ?? 0);

const canvas = document.querySelector("#screen");
const status = document.querySelector("#status");
const ctx = canvas.getContext("2d");
ctx.imageSmoothingEnabled = false;

const keys = new Set();
const solidTiles = new Set(level.solids.map((solid) => tileKey(solid.x, solid.y)));
const platformTiles = new Set((level.platforms ?? []).map((solid) => tileKey(solid.x, solid.y)));
const solidMaterialTiles = new Map(
  (level.solidMaterials ?? []).map((solid) => [tileKey(solid.x, solid.y), solid.kind]),
);
const visualEnemy = { initialized: false, x: 0, y: 0 };
const visualFlyingEnemy = { initialized: false, x: 0, y: 0 };
const visualFlyingEnemy2 = { initialized: false, x: 0, y: 0 };
let enemyDeathTime = null;
let flyingEnemyDeathTime = null;
let flyingEnemyDeathTime2 = null;
let screenMode = "title";
let startRequested = false;
let presentedLevel = 1;
let transitionUntil = 0;
let transitionFromLevel = 1;
let jumpQueued = false;
let jumpHeld = false;
let shootQueued = false;
let shootHeld = false;
let horizontalIntent = null;
let audioContext = null;
let lastAudioSeq = 0;
let jetpackToggleQueued = false;

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
      const key = tileKey(solid.x, solid.y);
      const material = solidMaterialTiles.get(key);
      if (platformTiles.has(key)) {
        drawPlatformTile(solid.x, solid.y, cameraX);
      } else if (material === "@") {
        drawMudTile(solid.x, solid.y, cameraX);
      } else if (material === "U") {
        drawBlueBlockTile(solid.x, solid.y, cameraX);
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
    const section = activeLevelSection(currentLevel);
    const enemyX = section.startX + (tape[MEM.ENEMY_X] || activeEnemy.x - section.startX);
    const enemyY = tape[MEM.ENEMY_Y] || activeEnemy.y;
    const enemyDraw = smoothEnemy(
      activeEnemy,
      enemyX,
      enemyY,
      tape[MEM.ENEMY_DIR],
      tape[MEM.ENEMY_TIMER],
    );
    if (tape[MEM.ENEMY_DEAD] !== 0) {
      if (enemyDeathTime === null) {
        enemyDeathTime = performance.now();
      }
      if (performance.now() - enemyDeathTime < 200) {
        drawEnemyBurst(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
      }
    } else {
      enemyDeathTime = null;
      drawEnemy(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
    }
  }

  const levelFlyingEnemies = (level.flyingEnemies ?? []).filter((e) => e.level === currentLevel);
  const section = activeLevelSection(currentLevel);

  if (levelFlyingEnemies[0]) {
    const enemy = levelFlyingEnemies[0];
    const enemyX = section.startX + (tape[MEM.FLYING_ENEMY_X] || (enemy.x - section.startX));
    const enemyY = tape[MEM.FLYING_ENEMY_Y] || (enemy.y - 1);
    const enemyDraw = smoothFlyingEnemy(
      enemy,
      enemyX,
      enemyY,
      tape[MEM.FLYING_ENEMY_DIR],
      tape[MEM.ENEMY_TIMER],
      visualFlyingEnemy,
    );
    const isDead = tape[MEM.FLYING_ENEMY_DEAD] !== 0;
    if (isDead) {
      if (flyingEnemyDeathTime === null) {
        flyingEnemyDeathTime = performance.now();
      }
      if (performance.now() - flyingEnemyDeathTime < 200) {
        drawEnemyBurst(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
      }
    } else {
      flyingEnemyDeathTime = null;
      drawFlyingEnemy(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
    }
  }

  if (levelFlyingEnemies[1]) {
    const enemy = levelFlyingEnemies[1];
    const enemyX = section.startX + (tape[MEM.FLYING_ENEMY2_X] || (enemy.x - section.startX));
    const enemyY = tape[MEM.FLYING_ENEMY2_Y] || (enemy.y - 1);
    const enemyDraw = smoothFlyingEnemy(
      enemy,
      enemyX,
      enemyY,
      tape[MEM.FLYING_ENEMY2_DIR],
      tape[MEM.ENEMY_TIMER],
      visualFlyingEnemy2,
    );
    const isDead = tape[MEM.FLYING_ENEMY2_DEAD] !== 0;
    if (isDead) {
      if (flyingEnemyDeathTime2 === null) {
        flyingEnemyDeathTime2 = performance.now();
      }
      if (performance.now() - flyingEnemyDeathTime2 < 200) {
        drawEnemyBurst(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
      }
    } else {
      flyingEnemyDeathTime2 = null;
      drawFlyingEnemy(VIEW_X + enemyDraw.x - cameraX, VIEW_Y + HUD_HEIGHT + enemyDraw.y);
    }
  }

  for (const gun of level.guns ?? []) {
    if (gun.level === currentLevel && tape[MEM.GUN_COLLECTED] === 0 && isInCamera(gun.x, cameraX)) {
      drawGunPickup(VIEW_X + gun.x * TILE - cameraX, VIEW_Y + HUD_HEIGHT + gun.y * TILE);
    }
  }

  (level.jetpacks ?? []).forEach((jetpack, index) => {
    if (jetpack.level === currentLevel && tape[JETPACK_BASE + index] === 0 && isInCamera(jetpack.x, cameraX)) {
      drawJetpackPickup(VIEW_X + jetpack.x * TILE - cameraX, VIEW_Y + HUD_HEIGHT + jetpack.y * TILE);
    }
  });

  if (tape[MEM.PROJECTILE_ACTIVE] !== 0) {
    const projectileX = worldTileX(currentLevel, tape[MEM.PROJECTILE_X]);
    drawProjectile(
      VIEW_X + projectileX * TILE - cameraX,
      VIEW_Y + HUD_HEIGHT + tape[MEM.PROJECTILE_Y] * TILE,
      tape[MEM.PROJECTILE_DIR] !== 0,
    );
  }

  if (tape[MEM.ENEMY_PROJ1_ACTIVE] !== 0) {
    const projX = worldTileX(currentLevel, tape[MEM.ENEMY_PROJ1_X]);
    drawEnemyProjectile(
      VIEW_X + projX * TILE - cameraX,
      VIEW_Y + HUD_HEIGHT + tape[MEM.ENEMY_PROJ1_Y] * TILE,
      tape[MEM.ENEMY_PROJ1_DIR] !== 0,
    );
  }

  if (tape[MEM.ENEMY_PROJ2_ACTIVE] !== 0) {
    const projX = worldTileX(currentLevel, tape[MEM.ENEMY_PROJ2_X]);
    drawEnemyProjectile(
      VIEW_X + projX * TILE - cameraX,
      VIEW_Y + HUD_HEIGHT + tape[MEM.ENEMY_PROJ2_Y] * TILE,
      tape[MEM.ENEMY_PROJ2_DIR] !== 0,
    );
  }

  const drawX = worldPixelX(currentLevel, tape[MEM.PLAYER_X], tape[MEM.PLAYER_SUB_X]);
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
    hasGun: tape[MEM.GUN_COLLECTED] !== 0,
    hasJetpack: tape[MEM.JETPACK_COLLECTED] !== 0,
    isJetpackActive: tape[MEM.JETPACK_ACTIVE] !== 0,
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
  const playerX = worldPixelX(
    tape[MEM.CURRENT_LEVEL] || 1,
    tape[MEM.PLAYER_X],
    tape[MEM.PLAYER_SUB_X],
  );
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

function worldTileX(currentLevel, localTileX) {
  return activeLevelSection(currentLevel).startX + localTileX;
}

function localTileX(currentLevel, worldTileXValue) {
  return worldTileXValue - activeLevelSection(currentLevel).startX;
}

function worldPixelX(currentLevel, localTileXValue, subX = 0) {
  return (
    (worldTileX(currentLevel, localTileXValue) * LOGICAL_TILE + subX) * RENDER_SCALE
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

  const door = tape[MEM.DOOR_OPEN] ? "OPEN" : "LOCK";
  const key = tape[MEM.KEY_COLLECTED] ? "YES" : "NO";
  const gun = tape[MEM.GUN_COLLECTED] ? "YES" : "NO";
  const currentLevel = tape[MEM.CURRENT_LEVEL] || 1;
  const restartPrompt = tape[MEM.GAME_WIN] !== 0 || tape[MEM.GAME_DEAD] !== 0;

  const yOffset = 12;

  drawPixelText("DAVE", VIEW_X + 12, VIEW_Y + yOffset, 1, "#45f35e", "#104018");
  drawPixelText(`SCORE ${scoreText(tape[MEM.SCORE])}`, VIEW_X + 60, VIEW_Y + yOffset, 1, "#dfffe4", "#23552a");
  drawPixelText(`LEVEL ${String(currentLevel).padStart(2, "0")}`, VIEW_X + 150, VIEW_Y + yOffset, 1, "#45f35e", "#104018");
  drawPixelText(`GUN ${gun}`, VIEW_X + 230, VIEW_Y + yOffset, 1, "#ffffff", "#333333");
  drawPixelText(`KEY ${key}`, VIEW_X + 295, VIEW_Y + yOffset, 1, "#55dfff", "#124451");
  drawPixelText(`DOOR ${door}`, VIEW_X + 355, VIEW_Y + yOffset, 1, "#ffe35a", "#4f3e0b");

  if (tape[MEM.GAME_WIN] !== 0) {
    ctx.fillStyle = "#030303";
    ctx.fillRect(VIEW_X + 420, VIEW_Y, SCREEN_WIDTH - 420, HUD_HEIGHT - 5);
    drawPixelText("DAVE WINS!", VIEW_X + 440, VIEW_Y + yOffset, 1, "#45f35e", "#104018");
  } else if (tape[MEM.JETPACK_COLLECTED] !== 0) {
    const fuelPct = Math.max(0, Math.min(1, tape[MEM.JETPACK_FUEL] / JETPACK_MAX_FUEL));
    const barWidth = Math.round(fuelPct * 70);
    const active = tape[MEM.JETPACK_ACTIVE] !== 0;

    drawPixelText("JET", VIEW_X + 430, VIEW_Y + yOffset, 1, active ? "#45f35e" : "#8a8a8a", active ? "#104018" : "#333333");

    ctx.fillStyle = active ? "#45f35e" : "#555555";
    ctx.fillRect(VIEW_X + 460, VIEW_Y + yOffset - 1, 74, 10);
    ctx.fillStyle = "#0c0c0c";
    ctx.fillRect(VIEW_X + 462, VIEW_Y + yOffset + 1, 70, 6);

    if (barWidth > 0) {
      let fillColor = "#20d040";
      if (fuelPct < 0.2) {
        const blink = Math.floor(performance.now() / 150) % 2 === 0;
        fillColor = blink ? "#ff2020" : "#600000";
      } else if (fuelPct < 0.5) {
        fillColor = "#ffa000";
      }
      ctx.fillStyle = fillColor;
      ctx.fillRect(VIEW_X + 462, VIEW_Y + yOffset + 1, barWidth, 6);
    }
  } else {
    if (restartPrompt) {
      drawPixelText("PRESS R TO RESTART", VIEW_X + 430, VIEW_Y + yOffset, 1, "#ff7777", "#4b0b0b");
    } else {
      drawPixelText("A BRAINFUCK GAME", VIEW_X + 470, VIEW_Y + yOffset, 1, "#ff7777", "#4b0b0b");
    }
  }
}

function drawTransitionScreen(fromLevel, toLevel, now, tape) {
  ctx.fillStyle = "#020202";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  drawHud({
    [MEM.SCORE]: tape?.[MEM.SCORE] ?? 0,
    [MEM.CURRENT_LEVEL]: toLevel,
    [MEM.KEY_COLLECTED]: 0,
    [MEM.DOOR_OPEN]: 0,
    [MEM.GAME_WIN]: 0,
    [MEM.GAME_DEAD]: 0,
    [MEM.GUN_COLLECTED]: tape?.[MEM.GUN_COLLECTED] ?? 0,
    [MEM.JETPACK_COLLECTED]: tape?.[MEM.JETPACK_COLLECTED] ?? 0,
    [MEM.JETPACK_ACTIVE]: tape?.[MEM.JETPACK_ACTIVE] ?? 0,
    [MEM.JETPACK_FUEL]: tape?.[MEM.JETPACK_FUEL] ?? 0,
  });

  const corridorY = VIEW_Y + HUD_HEIGHT + 120;
  for (let x = VIEW_X; x < VIEW_X + SCREEN_WIDTH; x += TILE) {
    drawTransitionBrick(x, corridorY - TILE);
    drawTransitionBrick(x, corridorY + TILE);
  }
  drawDoor(VIEW_X - 4, corridorY, true);
  const progress = Math.min(1, Math.max(0, (1800 - (transitionUntil - now)) / 1800));
  const daveX = VIEW_X + 90 + progress * (SCREEN_WIDTH - 100);
  const remLevels = level.levelCount - toLevel + 1;
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

function drawMudTile(tileX, tileY, cameraX) {
  const x = tileX * TILE;
  const y = VIEW_Y + HUD_HEIGHT + tileY * TILE;
  const drawX = VIEW_X + x - cameraX;
  const variant = (tileX * 5 + tileY * 9) % 4;

  ctx.fillStyle = ["#c47a43", "#d08348", "#b96d3d", "#cf8550"][variant];
  ctx.fillRect(drawX, y, TILE, TILE);
  ctx.fillStyle = "#6f4528";
  for (let row = 1; row < 3; row += 1) {
    const py = y + row * 10 + ((tileX + tileY) % 3) - 9;
    ctx.fillRect(drawX + 2, py + 4, 10, 2);
    ctx.fillRect(drawX + 10, py + 9, 14, 2);
    ctx.fillRect(drawX + 23, py + 2, 8, 2);
    ctx.fillRect(drawX + 7, py + 1, 2, 7);
    ctx.fillRect(drawX + 20, py + 7, 2, 8);
  }
  ctx.fillStyle = "#e0a06c";
  ctx.fillRect(drawX + 2, y + 2, TILE - 4, 2);
  ctx.fillRect(drawX + 5, y + 13, 9, 2);
  ctx.fillRect(drawX + 18, y + 23, 10, 2);
  ctx.fillStyle = "#4f2f1f";
  ctx.fillRect(drawX, y + TILE - 3, TILE, 3);
  if (!isSolid(tileX - 1, tileY)) {
    ctx.fillStyle = "#e49458";
    ctx.fillRect(drawX, y, 3, TILE);
  }
  if (!isSolid(tileX + 1, tileY)) {
    ctx.fillStyle = "#5a3422";
    ctx.fillRect(drawX + TILE - 3, y, 3, TILE);
  }
}

function drawBlueBlockTile(tileX, tileY, cameraX) {
  const x = tileX * TILE;
  const y = VIEW_Y + HUD_HEIGHT + tileY * TILE;
  const drawX = VIEW_X + x - cameraX;
  const shimmer = (tileX * 3 + tileY * 5) % 8;

  ctx.fillStyle = "#0872c8";
  ctx.fillRect(drawX, y, TILE, TILE);
  ctx.fillStyle = "#12a7f5";
  ctx.fillRect(drawX, y + 3, TILE, 6);
  ctx.fillRect(drawX, y + 17, TILE, 6);
  ctx.fillStyle = "#003d83";
  ctx.fillRect(drawX, y + 9, TILE, 3);
  ctx.fillRect(drawX, y + 25, TILE, 3);
  ctx.fillStyle = "#6ee7ff";
  ctx.fillRect(drawX + shimmer, y + 4, 6, 5);
  ctx.fillRect(drawX + 16 + shimmer / 2, y + 18, 7, 5);
  ctx.fillStyle = "#001f4d";
  ctx.fillRect(drawX, y + TILE - 3, TILE, 3);
  if (!isSolid(tileX - 1, tileY)) {
    ctx.fillStyle = "#25c6ff";
    ctx.fillRect(drawX, y, 3, TILE);
  }
  if (!isSolid(tileX + 1, tileY)) {
    ctx.fillStyle = "#003064";
    ctx.fillRect(drawX + TILE - 3, y, 3, TILE);
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

function drawEnemyBurst(x, y) {
  ctx.fillStyle = "#ff2fb3";
  ctx.fillRect(x + 8, y + 15, 5, 5);
  ctx.fillRect(x + 19, y + 15, 5, 5);
  ctx.fillStyle = "#ffe4fb";
  ctx.fillRect(x + 14, y + 9, 4, 4);
  ctx.fillRect(x + 14, y + 25, 4, 4);
  ctx.fillStyle = "#7a0b72";
  ctx.fillRect(x + 10, y + 22, 3, 3);
  ctx.fillRect(x + 21, y + 22, 3, 3);
}

function drawFlyingEnemy(x, y) {
  const now = performance.now();
  const armFrame = Math.floor(now / 150) % 4;

  const flyingEnemyColors = {
    '.': null,
    'k': '#0d0b0e',
    's': '#3a4d59',
    'm': '#738c9c',
    'l': '#b2c5d1',
    'w': '#ffffff',
    'r': '#ff1a40',
    'p': '#ff8093',
    'o': '#ff5500',
    'y': '#ffcc00',
    'g': '#4e5a60',
    'h': '#dce5e7',
    'b': '#8a9ea7',
    'j': '#5c6e78',
    'c': '#c8d6dc',
  };

  function drawPixelGrid(grid, startCol, startRow, mirror = false) {
    for (let r = 0; r < grid.length; r++) {
      const rowStr = grid[r];
      for (let c = 0; c < rowStr.length; c++) {
        const char = mirror ? rowStr[rowStr.length - 1 - c] : rowStr[c];
        const color = flyingEnemyColors[char];
        if (color) {
          ctx.fillStyle = color;
          ctx.fillRect(x + startCol + c, y + startRow + r, 1, 1);
        }
      }
    }
  }

  if (armFrame === 0) {
    const leftUpperArm = [
      "..kk......",
      ".kbhk.....",
      "kbgk......",
      ".kbhk.....",
      "..kbbk....",
      "...kbgk...",
      "....kck...",
    ];
    const leftLowerArm = [
      "....kck...",
      "...kbgk...",
      "..kbbk....",
      ".kbhk.....",
      "kbgk......",
      ".kk.......",
    ];
    drawPixelGrid(leftUpperArm, 1, 4, false);
    drawPixelGrid(leftUpperArm, 22, 4, true);
    drawPixelGrid(leftLowerArm, 1, 16, false);
    drawPixelGrid(leftLowerArm, 22, 16, true);
  } else if (armFrame === 1) {
    const leftUpperArm = [
      "..........",
      "kkkkkk....",
      "khhbbgk...",
      "kckkbgk...",
      "....kkk...",
    ];
    const leftLowerArm = [
      "....kkk...",
      "kckkbgk...",
      "khhbbgk...",
      "kkkkkk....",
    ];
    drawPixelGrid(leftUpperArm, 0, 7, false);
    drawPixelGrid(leftUpperArm, 23, 7, true);
    drawPixelGrid(leftLowerArm, 0, 17, false);
    drawPixelGrid(leftLowerArm, 23, 17, true);
  } else if (armFrame === 2) {
    const leftUpperArm = [
      "....kck...",
      "...kbgk...",
      "..kbbk....",
      ".kbhk.....",
      "kbgk......",
      ".kbhk.....",
      "..kk......",
    ];
    const leftLowerArm = [
      ".kk.......",
      "kbgk......",
      ".kbhk.....",
      "..kbbk....",
      "...kbgk...",
      "....kck...",
    ];
    drawPixelGrid(leftUpperArm, 1, 5, false);
    drawPixelGrid(leftUpperArm, 22, 5, true);
    drawPixelGrid(leftLowerArm, 1, 15, false);
    drawPixelGrid(leftLowerArm, 22, 15, true);
  } else {
    const leftUpperArm = [
      "kck.......",
      ".kbgk.....",
      "..kbbk....",
      "...kbhk...",
      "....kbgk..",
      ".....kkk..",
    ];
    const leftLowerArm = [
      ".....kkk..",
      "....kbgk..",
      "...kbhk...",
      "..kbbk....",
      ".kbgk.....",
      "kck.......",
    ];
    drawPixelGrid(leftUpperArm, 0, 5, false);
    drawPixelGrid(leftUpperArm, 23, 5, true);
    drawPixelGrid(leftLowerArm, 0, 16, false);
    drawPixelGrid(leftLowerArm, 23, 16, true);
  }

  const flyingEnemyBodyGrid = [
    "....kkkkkkk....",
    "..kkllmsssskk..",
    ".klllmmmmsssssk.",
    "klwllkkkkkkksssk",
    "klwlkrrrrrrrksks",
    "klmkrppppppprkss",
    "klmkrppwkppprkss",
    "klmkrppkkkpprkss",
    "klmkrppppppprkss",
    "klwlkrrrrrrrksks",
    "klwllkkkkkkksssk",
    ".kmmmmkkkksssk.",
    "..kkkkkkkkkkk..",
  ];
  drawPixelGrid(flyingEnemyBodyGrid, 9, 8, false);

  const flameFrame = Math.floor(now / 80) % 3;
  if (flameFrame === 0) {
    ctx.fillStyle = "#ff5500";
    ctx.fillRect(x + 15, y + 21, 4, 1);
    ctx.fillStyle = "#ffcc00";
    ctx.fillRect(x + 16, y + 22, 2, 1);
  } else if (flameFrame === 1) {
    ctx.fillStyle = "#ff2a00";
    ctx.fillRect(x + 14, y + 21, 6, 1);
    ctx.fillStyle = "#ff5500";
    ctx.fillRect(x + 15, y + 22, 4, 1);
    ctx.fillStyle = "#ffcc00";
    ctx.fillRect(x + 16, y + 23, 2, 1);
  } else {
    ctx.fillStyle = "#ff5500";
    ctx.fillRect(x + 15, y + 21, 4, 1);
    ctx.fillStyle = "#ffcc00";
    ctx.fillRect(x + 16, y + 22, 2, 1);
  }
}

function drawGunPickup(x, y) {
  ctx.fillStyle = "#6b3518";
  ctx.fillRect(x + 10, y + 18, 5, 8);
  ctx.fillStyle = "#d9d9d9";
  ctx.fillRect(x + 10, y + 12, 14, 5);
  ctx.fillStyle = "#f4f4f4";
  ctx.fillRect(x + 12, y + 10, 7, 2);
  ctx.fillStyle = "#5d5d5d";
  ctx.fillRect(x + 21, y + 14, 6, 2);
  ctx.fillRect(x + 14, y + 17, 4, 3);
  ctx.fillStyle = "#2b2b2b";
  ctx.fillRect(x + 16, y + 20, 3, 2);
}

function drawJetpackPickup(x, y) {
  const steelDark = "#126b27";
  const steelMid = "#2baf4a";
  const steelLight = "#6bff8f";
  const steelHighlight = "#ffffff";
  const darkMetal = "#1f2226";
  const copper = "#ad7222";
  const copperDark = "#5e3e11";
  
  ctx.fillStyle = copperDark;
  ctx.fillRect(x + 8, y + 24, 4, 3);
  ctx.fillRect(x + 20, y + 24, 4, 3);
  
  ctx.fillStyle = steelMid;
  ctx.fillRect(x + 8, y + 6, 6, 18);
  ctx.fillStyle = steelLight;
  ctx.fillRect(x + 8, y + 6, 2, 18);
  ctx.fillStyle = steelHighlight;
  ctx.fillRect(x + 8, y + 7, 1, 6);
  ctx.fillStyle = steelDark;
  ctx.fillRect(x + 12, y + 6, 2, 18);
  
  ctx.fillStyle = steelLight;
  ctx.fillRect(x + 9, y + 4, 4, 2);
  
  ctx.fillStyle = steelMid;
  ctx.fillRect(x + 18, y + 6, 6, 18);
  ctx.fillStyle = steelLight;
  ctx.fillRect(x + 18, y + 6, 2, 18);
  ctx.fillStyle = steelHighlight;
  ctx.fillRect(x + 18, y + 7, 1, 6);
  ctx.fillStyle = steelDark;
  ctx.fillRect(x + 22, y + 6, 2, 18);
  
  ctx.fillStyle = steelLight;
  ctx.fillRect(x + 19, y + 4, 4, 2);
  
  ctx.fillStyle = copper;
  ctx.fillRect(x + 14, y + 12, 4, 2);
  ctx.fillRect(x + 15, y + 14, 2, 2);
  
  ctx.fillStyle = darkMetal;
  ctx.fillRect(x + 7, y + 8, 18, 2);
  ctx.fillRect(x + 7, y + 18, 18, 2);
  
  ctx.fillStyle = "#ffcc00";
  ctx.fillRect(x + 15, y + 10, 2, 2);
}

function drawProjectile(x, y, right) {
  const px = x + (right ? 30 : -8);
  const py = y + 20;
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(px, py, 9, 3);
  ctx.fillStyle = "#ffd45c";
  ctx.fillRect(right ? px - 3 : px + 9, py + 1, 3, 1);
}

function drawEnemyProjectile(x, y, right) {
  const px = x + (right ? 30 : -8);
  const py = y + 20;
  const now = performance.now();
  const pulse = Math.floor(now / 50) % 2 === 0;

  ctx.fillStyle = "#8b0000";
  ctx.fillRect(px, py, 7, 3);

  ctx.fillStyle = "#ff5500";
  ctx.fillRect(px + 1, py + 1, 5, 1);

  ctx.fillStyle = "#ffcc00";
  ctx.fillRect(px + 2, py + 1, 3, 1);

  ctx.fillStyle = pulse ? "#ff5500" : "#ffcc00";
  ctx.fillRect(right ? px - 2 : px + 7, py + 1, 1, 1);
  ctx.fillStyle = pulse ? "#ffcc00" : "#ff5500";
  ctx.fillRect(right ? px - 4 : px + 9, py + (pulse ? 0 : 2), 1, 1);
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

  if (state.hasGun) {
    spriteRect(13, 10, 3, 1, "#dcdcdc");
    spriteRect(15, 10, 1, 1, "#ffffff");
    spriteRect(13, 11, 1, 2, "#4d4d4d");
    spriteRect(12, 12, 2, 1, "#2a2a2a");
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

  if (state.hasJetpack && !state.dead) {
    const strapColor = "#55555d";
    const darkMetal = "#1f2226";
    const steelDark = "#126b27";
    const steelMid = "#2baf4a";
    const steelLight = "#6bff8f";
    const steelHighlight = "#ffffff";
    const copper = "#ad7222";
    const copperDark = "#5e3e11";
    
    spriteRect(5, 10, 1, 1, strapColor);
    spriteRect(5, 11, 2, 1, strapColor);

    spriteRect(1, 9, 2, 5, steelDark);
    spriteRect(1, 8, 1, 1, steelDark);
    
    spriteRect(3, 8, 2, 6, steelMid);
    spriteRect(3, 8, 1, 6, steelLight);
    spriteRect(3, 9, 1, 3, steelHighlight);
    spriteRect(4, 8, 1, 6, steelDark);
    spriteRect(3, 7, 1, 1, steelLight);
    
    spriteRect(2, 9, 2, 1, darkMetal);
    spriteRect(2, 12, 2, 1, darkMetal);

    spriteRect(2, 10, 1, 2, copper);
    spriteRect(3, 11, 1, 1, "#ff9900");

    spriteRect(1, 14, 1, 1, copperDark);
    spriteRect(4, 14, 1, 1, copperDark);

    const active = state.isJetpackActive;
    if (active) {
      const blink = Math.floor(performance.now() / 120) % 2 === 0;
      spriteRect(3, 10, 1, 1, blink ? "#3bf2ff" : "#095861");
      spriteRect(4, 12, 1, 1, blink ? "#ff2a2a" : "#610909");
    } else {
      spriteRect(3, 10, 1, 1, "#042c30");
      spriteRect(4, 12, 1, 1, "#400606");
    }

    if (active) {
      const isMoving = keys.has("ArrowUp") || keys.has("ArrowDown");
      const frame = Math.floor(performance.now() / 50) % 3;
      
      if (isMoving) {
        if (frame === 0) {
          spriteRect(1, 15, 1, 2, "#ffffff");
          spriteRect(0, 16, 2, 3, "#ffd43f");
          spriteRect(0, 18, 1, 2, "#ff7c00");
          spriteRect(4, 15, 1, 2, "#ffffff");
          spriteRect(4, 16, 2, 3, "#ffd43f");
          spriteRect(5, 18, 1, 2, "#ff7c00");
        } else if (frame === 1) {
          spriteRect(0, 15, 2, 3, "#ffffff");
          spriteRect(0, 17, 2, 2, "#ffd43f");
          spriteRect(0, 19, 1, 3, "#ff7c00");
          spriteRect(4, 15, 2, 3, "#ffffff");
          spriteRect(4, 17, 2, 2, "#ffd43f");
          spriteRect(5, 19, 1, 3, "#ff7c00");
        } else {
          spriteRect(1, 15, 1, 2, "#ffffff");
          spriteRect(0, 16, 2, 2, "#ffd43f");
          spriteRect(0, 18, 2, 2, "#ff7c00");
          spriteRect(0, 20, 1, 2, "#ff2020");
          spriteRect(4, 15, 1, 2, "#ffffff");
          spriteRect(4, 16, 2, 2, "#ffd43f");
          spriteRect(4, 18, 2, 2, "#ff7c00");
          spriteRect(5, 20, 1, 2, "#ff2020");
        }
      } else {
        if (frame === 0) {
          spriteRect(1, 15, 1, 2, "#ffffff");
          spriteRect(4, 15, 1, 2, "#ffffff");
        } else if (frame === 1) {
          spriteRect(1, 15, 1, 1, "#ffd43f");
          spriteRect(0, 16, 1, 1, "#ff7c00");
          spriteRect(4, 15, 1, 1, "#ffd43f");
          spriteRect(5, 16, 1, 1, "#ff7c00");
        } else {
          spriteRect(1, 15, 1, 2, "#ffd43f");
          spriteRect(4, 15, 1, 2, "#ffd43f");
        }
      }
    }
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

function smoothFlyingEnemy(enemy, tileX, tileY, direction, timer, visState) {
  const startX = tileX * TILE;
  const startY = tileY * TILE;
  if (!visState.initialized) {
    visState.initialized = true;
    visState.x = startX;
    visState.y = startY;
  }

  const delay = 18;
  const clampedTimer = Math.max(0, Math.min(delay, timer));
  const progress = (delay - clampedTimer) / delay;
  let targetTileX = tileX;
  let targetTileY = tileY;

  if (direction === 0) {
    targetTileX = tileX + 1;
    targetTileY = tileY + 1;
  } else if (direction === 1) {
    targetTileX = tileX - 1;
    targetTileY = tileY + 1;
  } else if (direction === 2) {
    targetTileX = tileX - 1;
    targetTileY = tileY - 1;
  } else if (direction === 3) {
    targetTileX = tileX + 1;
    targetTileY = tileY - 1;
  }

  const targetX = targetTileX * TILE;
  const targetY = targetTileY * TILE;
  visState.x = startX + (targetX - startX) * progress;
  visState.y = startY + (targetY - startY) * progress;

  return {
    x: Math.round(visState.x),
    y: Math.round(visState.y),
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
  } else if (event === 7) {
    playShootSound();
  } else if (event === 8) {
    playHitSound();
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

function playShootSound() {
  tone(1180, 0.025, "square", 0.055, -120);
  tone(520, 0.045, "sawtooth", 0.038, -260, 0.008);
  noise(0.035, 0.035, 0, "highpass", 2600);
}

function playHitSound() {
  noise(0.08, 0.06, 0, "bandpass", 1200);
  tone(160, 0.08, "square", 0.05, -120);
  arpeggio([360, 290, 220], 0.028, "triangle", 0.035);
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
    event.code === "ArrowDown" ||
    event.code === "AltLeft" ||
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

    if (event.code === "AltLeft" && !keys.has("AltLeft")) {
      jetpackToggleQueued = true;
    }

    keys.add(event.code);
    if (event.code === "ArrowUp" && !jumpHeld) {
      jumpQueued = true;
      jumpHeld = true;
    }
    if (event.code === "Space" && !shootHeld) {
      shootQueued = true;
      shootHeld = true;
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
  if (event.code === "ArrowUp") {
    jumpHeld = keys.has("ArrowUp");
  }
  if (event.code === "Space") {
    shootHeld = false;
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
  vm.tape[MEM.PLAYER_X] = localTileX(1, level.playerStart.x);
  vm.tape[MEM.PLAYER_Y] = level.playerStart.y;
  vm.tape[MEM.PLAYER_FACING] = 1;
  const firstEnemy = entityForLevel(level.enemies, 1);
  vm.tape[MEM.ENEMY_X] = firstEnemy ? localTileX(1, firstEnemy.x) : 0;
  vm.tape[MEM.ENEMY_Y] = firstEnemy?.y ?? 0;
  vm.tape[MEM.ENEMY_DIR] = 1;

  function tickBrainfuck() {
    const jetpackActive = vm.tape[MEM.JETPACK_ACTIVE] !== 0;
    const jumpInput = jetpackActive ? (keys.has("ArrowUp") ? 1 : 0) : (jumpQueued ? 1 : 0);
    const downInput = jetpackActive ? (keys.has("ArrowDown") ? 1 : 0) : 0;
    const shootInput = shootQueued ? 1 : 0;
    const jetpackToggleInput = jetpackToggleQueued ? 1 : 0;
    jumpQueued = false;
    shootQueued = false;
    jetpackToggleQueued = false;

    vm.tape[MEM.INPUT_RIGHT] = horizontalIntent === "ArrowRight" ? 1 : 0;
    vm.tape[MEM.INPUT_LEFT] = horizontalIntent === "ArrowLeft" ? 1 : 0;
    vm.tape[MEM.INPUT_JUMP] = jumpInput;
    vm.tape[MEM.INPUT_DOWN] = downInput;
    vm.tape[MEM.INPUT_SHOOT] = shootInput;
    vm.tape[MEM.INPUT_JETPACK_TOGGLE] = jetpackToggleInput;
    vm.tape[MEM.INPUT_RESTART] = keys.has("KeyR") ? 1 : 0;
    vm.tape[MEM.TICK_DONE] = 0;
    vm.tape[MEM.TICK_REQUESTED] = 1;
    vm.rewind();
    return vm.runUntil((machine) => machine.tape[MEM.TICK_DONE] === 1, 1000000);
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

    if (transitionUntil > now && presentedLevel > transitionFromLevel) {
      accumulator = 0;
      previousTime = now;
      drawTransitionScreen(transitionFromLevel, presentedLevel, now, vm.tape);
      requestAnimationFrame(frame);
      return;
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
        visualEnemy.initialized = false;
        visualFlyingEnemy.initialized = false;
        visualFlyingEnemy2.initialized = false;
        enemyDeathTime = null;
        flyingEnemyDeathTime = null;
        flyingEnemyDeathTime2 = null;
      }
      accumulator -= STEP_MS;
    }

    if (transitionUntil > now && presentedLevel > transitionFromLevel) {
      drawTransitionScreen(transitionFromLevel, presentedLevel, now, vm.tape);
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
