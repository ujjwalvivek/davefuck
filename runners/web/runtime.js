import { BrainfuckVM, MEM } from "./vm.js";
import { level } from "/game/generated/level.js";

const TILE = 32;
const LOGICAL_TILE = 16;
const RENDER_SCALE = TILE / LOGICAL_TILE;
const STEP_MS = 1000 / 60;
const MAX_FRAME_MS = 100;

const canvas = document.querySelector("#screen");
const status = document.querySelector("#status");
const ctx = canvas.getContext("2d");
ctx.imageSmoothingEnabled = false;

const keys = new Set();
let horizontalIntent = null;

function resize() {
  const scale = Math.max(
    1,
    Math.floor(
      Math.min(
        window.innerWidth / (level.width * TILE),
        window.innerHeight / (level.height * TILE),
      ),
    ),
  );
  canvas.width = level.width * TILE;
  canvas.height = level.height * TILE;
  canvas.style.width = `${level.width * TILE * scale}px`;
  canvas.style.height = `${level.height * TILE * scale}px`;
  ctx.imageSmoothingEnabled = false;
}

function draw(tape) {
  ctx.fillStyle = "#0c0c0c";
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  for (const solid of level.solids) {
    drawBrickTile(solid.x * TILE, solid.y * TILE);
  }

  drawExit(level.exit.x * TILE, level.exit.y * TILE, tape[MEM.GAME_WIN] !== 0);
  if (tape[MEM.KEY_COLLECTED] === 0) {
    drawKey(level.key.x * TILE, level.key.y * TILE);
  }
  if (tape[MEM.DOOR_OPEN] === 0) {
    drawDoor(level.door.x * TILE, level.door.y * TILE);
  }

  const enemyX = tape[MEM.ENEMY_X] || level.enemy.x;
  const enemyY = tape[MEM.ENEMY_Y] || level.enemy.y;
  drawEnemy(enemyX * TILE, enemyY * TILE);

  const drawX =
    (tape[MEM.PLAYER_X] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_X]) * RENDER_SCALE;
  const drawY =
    (tape[MEM.PLAYER_Y] * LOGICAL_TILE + tape[MEM.PLAYER_SUB_Y]) * RENDER_SCALE;
  drawDave(drawX, drawY, tape[MEM.GAME_DEAD] !== 0);

  if (status) {
    status.textContent =
      tape[MEM.GAME_WIN] !== 0
        ? "WIN - press R"
        : tape[MEM.GAME_DEAD] !== 0
          ? "DEAD - press R"
          : "";
  }
}

function drawBrickTile(x, y) {
  ctx.fillStyle = "#d30905";
  ctx.fillRect(x, y, TILE, TILE);
  ctx.fillStyle = "#8b0806";
  ctx.fillRect(x, y + 7, TILE, 2);
  ctx.fillRect(x, y + 15, TILE, 2);
  ctx.fillRect(x, y + 23, TILE, 2);
  ctx.fillRect(x + 15, y, 2, 8);
  ctx.fillRect(x + 7, y + 8, 2, 8);
  ctx.fillRect(x + 23, y + 16, 2, 8);
  ctx.fillStyle = "#ff2a1d";
  ctx.fillRect(x + 2, y + 2, TILE - 4, 2);
}

function drawKey(x, y) {
  ctx.fillStyle = "#ffd84d";
  ctx.beginPath();
  ctx.moveTo(x + 16, y + 5);
  ctx.lineTo(x + 25, y + 14);
  ctx.lineTo(x + 16, y + 23);
  ctx.lineTo(x + 7, y + 14);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = "#fff3a5";
  ctx.fillRect(x + 14, y + 8, 4, 4);
}

function drawDoor(x, y) {
  ctx.fillStyle = "#9f6432";
  ctx.fillRect(x + 5, y + 2, 22, 30);
  ctx.fillStyle = "#6d3c20";
  ctx.fillRect(x + 8, y + 5, 3, 24);
  ctx.fillRect(x + 20, y + 5, 3, 24);
  ctx.fillStyle = "#ffd84d";
  ctx.fillRect(x + 22, y + 16, 3, 3);
}

function drawExit(x, y, active) {
  ctx.fillStyle = active ? "#84f7ff" : "#2f7380";
  ctx.fillRect(x + 6, y + 3, 20, 26);
  ctx.fillStyle = "#101010";
  ctx.fillRect(x + 10, y + 7, 12, 18);
}

function drawEnemy(x, y) {
  ctx.fillStyle = "#d21cff";
  ctx.fillRect(x + 6, y + 12, 20, 12);
  ctx.fillStyle = "#7b0f9b";
  ctx.fillRect(x + 6, y + 24, 5, 4);
  ctx.fillRect(x + 21, y + 24, 5, 4);
  ctx.fillStyle = "#fff2ff";
  ctx.fillRect(x + 10, y + 15, 4, 4);
  ctx.fillRect(x + 18, y + 15, 4, 4);
}

function drawDave(x, y, dead) {
  const s = RENDER_SCALE;
  const px = x + 3 * s;
  const py = y + 2 * s;

  ctx.fillStyle = "#f1d06a";
  ctx.fillRect(px + 2 * s, py, 6 * s, 2 * s);
  ctx.fillStyle = "#f2f0df";
  ctx.fillRect(px + 1 * s, py + 2 * s, 8 * s, 7 * s);
  ctx.fillStyle = "#34515a";
  ctx.fillRect(px + 2 * s, py + 4 * s, 2 * s, 2 * s);
  ctx.fillRect(px + 6 * s, py + 4 * s, 2 * s, 2 * s);
  ctx.fillStyle = "#2a2a2a";
  ctx.fillRect(px + 4 * s, py + 7 * s, 3 * s, s);
  ctx.fillStyle = "#4aa0b5";
  ctx.fillRect(px + 2 * s, py + 9 * s, 6 * s, 4 * s);
  ctx.fillStyle = "#1f1f1f";
  ctx.fillRect(px + 1 * s, py + 13 * s, 3 * s, s);
  ctx.fillRect(px + 6 * s, py + 13 * s, 3 * s, s);
  if (dead) {
    ctx.fillStyle = "#c51f1f";
    ctx.fillRect(px + 2 * s, py + 4 * s, 6 * s, s);
  }
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
