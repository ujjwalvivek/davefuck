import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";
import { level } from "../../game/generated/level.js";
import { BrainfuckVM, MEM } from "./vm.js";

const TILE = 16;
const ROM = fs.readFileSync("rom/dave.bf", "utf8");

function sectionFor(currentLevel) {
  return level.sections.find((section) => section.kind === "level" && section.level === currentLevel);
}

function localX(currentLevel, worldX) {
  return worldX - sectionFor(currentLevel).startX;
}

function createHostVM(currentLevel = 1) {
  const vm = new BrainfuckVM(ROM, { tapeSize: 512 });
  vm.tape[MEM.PLAYER_X] = localX(1, level.playerStart.x);
  vm.tape[MEM.PLAYER_Y] = level.playerStart.y;
  vm.tape[MEM.PLAYER_FACING] = 1;
  const enemy = level.enemies.find((entity) => entity.level === currentLevel);
  vm.tape[MEM.ENEMY_X] = enemy ? localX(currentLevel, enemy.x) : 0;
  vm.tape[MEM.ENEMY_Y] = enemy?.y ?? 0;
  vm.tape[MEM.ENEMY_DIR] = 1;
  vm.tape[MEM.CURRENT_LEVEL] = currentLevel;
  vm.tape[MEM.GAME_STARTED] = 1;

  const lvlEnemies = level.flyingEnemies.filter((e) => e.level === currentLevel);
  if (lvlEnemies.length === 0) {
    vm.tape[MEM.FLYING_ENEMY_DEAD] = 1;
    vm.tape[MEM.FLYING_ENEMY2_DEAD] = 1;
  } else if (lvlEnemies.length === 1) {
    const e = lvlEnemies[0];
    vm.tape[MEM.FLYING_ENEMY_X] = localX(currentLevel, e.x);
    vm.tape[MEM.FLYING_ENEMY_Y] = e.y;
    vm.tape[MEM.FLYING_ENEMY_DEAD] = 0;
    vm.tape[MEM.FLYING_ENEMY2_DEAD] = 1;
  } else {
    const e1 = lvlEnemies[0];
    const e2 = lvlEnemies[1];
    vm.tape[MEM.FLYING_ENEMY_X] = localX(currentLevel, e1.x);
    vm.tape[MEM.FLYING_ENEMY_Y] = e1.y;
    vm.tape[MEM.FLYING_ENEMY_DEAD] = 0;
    vm.tape[MEM.FLYING_ENEMY2_X] = localX(currentLevel, e2.x);
    vm.tape[MEM.FLYING_ENEMY2_Y] = e2.y;
    vm.tape[MEM.FLYING_ENEMY2_DEAD] = 0;
  }
  vm.tape[MEM.FLYING_ENEMY_SHOOT_TIMER] = 150;
  vm.tape[MEM.FLYING_ENEMY2_SHOOT_TIMER] = 150;

  return vm;
}

function tick(
  vm,
  { left = 0, right = 0, jump = 0, down = 0, shoot = 0, jetpack = 0, restart = 0 } = {},
) {
  vm.tape[MEM.INPUT_LEFT] = left;
  vm.tape[MEM.INPUT_RIGHT] = right;
  vm.tape[MEM.INPUT_JUMP] = jump;
  vm.tape[MEM.INPUT_DOWN] = down;
  vm.tape[MEM.INPUT_SHOOT] = shoot;
  vm.tape[MEM.INPUT_JETPACK_TOGGLE] = jetpack;
  vm.tape[MEM.INPUT_RESTART] = restart;
  vm.tape[MEM.TICK_DONE] = 0;
  vm.tape[MEM.TICK_REQUESTED] = 1;
  vm.rewind();
  return vm.runUntil((machine) => machine.tape[MEM.TICK_DONE] === 1, 1000000);
}

function pixelY(vm) {
  return vm.tape[MEM.PLAYER_Y] * TILE + vm.tape[MEM.PLAYER_SUB_Y];
}

function finalDoor() {
  return level.doors.find((door) => door.level === level.levelCount);
}

test("browser host VM can run the generated ROM for one tick", () => {
  const vm = createHostVM();

  const steps = tick(vm);

  assert.equal(vm.tape[MEM.TICK_DONE], 1);
  assert.ok(steps > 0);
  assert.equal(vm.tape[MEM.PLAYER_X], localX(1, level.playerStart.x));
  assert.equal(vm.tape[MEM.PLAYER_Y], level.playerStart.y);
});

test("browser host observes BF-owned horizontal movement", () => {
  const vm = createHostVM();

  tick(vm, { right: 1 });

  assert.equal(vm.tape[MEM.PLAYER_X], localX(1, level.playerStart.x));
  assert.equal(vm.tape[MEM.PLAYER_SUB_X], 1);
});

test("browser host observes BF-owned facing state", () => {
  const vm = createHostVM();

  tick(vm, { left: 1 });
  assert.equal(vm.tape[MEM.PLAYER_FACING], 0);

  tick(vm, { right: 1 });
  assert.equal(vm.tape[MEM.PLAYER_FACING], 1);
});

test("browser host observes BF-owned jump state", () => {
  const vm = createHostVM();

  tick(vm, { jump: 1 });

  assert.equal(pixelY(vm), level.playerGroundY * TILE - 2);
  assert.equal(vm.tape[MEM.PLAYER_JUMP_PHASE], 1);
  assert.equal(vm.tape[MEM.PLAYER_JUMP_TIMER], 20);
});

test("browser host diagonal input finishes within the browser step budget", () => {
  const vm = createHostVM();
  vm.tape[MEM.PLAYER_X] = sectionFor(1).width - 3;

  const steps = tick(vm, { right: 1, jump: 1 });

  assert.equal(vm.tape[MEM.TICK_DONE], 1);
  assert.ok(steps < 1000000);
});

test("browser host squeeze collision finishes within the browser step budget", () => {
  const vm = createHostVM();
  vm.tape[MEM.PLAYER_X] = 4;
  vm.tape[MEM.PLAYER_Y] = 6;
  vm.tape[MEM.PLAYER_SUB_X] = 2;
  vm.tape[MEM.PLAYER_SUB_Y] = 0;
  vm.tape[MEM.PLAYER_JUMP_PHASE] = 2;

  const steps = tick(vm);

  assert.equal(vm.tape[MEM.TICK_DONE], 1);
  assert.equal(vm.tape[MEM.PLAYER_JUMP_PHASE], 0);
  assert.ok(steps < 1000000);
});

test("browser host observes BF-owned key and door state", () => {
  const vm = createHostVM();
  vm.tape[MEM.PLAYER_X] = localX(1, level.key.x);
  vm.tape[MEM.PLAYER_Y] = level.key.y;

  tick(vm);

  assert.equal(vm.tape[MEM.KEY_COLLECTED], 1);
  assert.equal(vm.tape[MEM.DOOR_OPEN], 1);
  assert.equal(vm.tape[MEM.AUDIO_EVENT], 4);
});

test("browser host observes BF-owned coins and score", () => {
  const vm = createHostVM();
  const coin = level.coins.find((item) => item.level === 1) ?? level.coins[0];
  vm.tape[MEM.CURRENT_LEVEL] = coin.level;
  vm.tape[MEM.PLAYER_X] = localX(coin.level, coin.x);
  vm.tape[MEM.PLAYER_Y] = coin.y;

  tick(vm);

  assert.equal(vm.tape[MEM.COIN_BASE], 1);
  assert.equal(vm.tape[MEM.SCORE], 10);
  assert.equal(vm.tape[MEM.AUDIO_EVENT], 3);
  assert.equal(vm.tape[MEM.AUDIO_SEQ], 1);

  tick(vm);

  assert.equal(vm.tape[MEM.COIN_BASE], 1);
  assert.equal(vm.tape[MEM.SCORE], 10);
  assert.equal(vm.tape[MEM.AUDIO_SEQ], 1);
});

test("browser host observes BF-owned gun and projectile state", () => {
  const gun = level.guns.find((item) => item.level === 3) ?? level.guns[0];
  const vm = createHostVM(gun.level);
  vm.tape[MEM.PLAYER_X] = localX(gun.level, gun.x);
  vm.tape[MEM.PLAYER_Y] = gun.y;

  tick(vm);

  assert.equal(vm.tape[MEM.GUN_COLLECTED], 1);

  tick(vm, { shoot: 1 });

  assert.equal(vm.tape[MEM.PROJECTILE_ACTIVE], 1);
  assert.equal(vm.tape[MEM.PROJECTILE_Y], gun.y);
  assert.equal(vm.tape[MEM.AUDIO_EVENT], 7);
});

test("browser host observes BF-owned jetpack fuel and toggle state", () => {
  const jetpack = level.jetpacks[0];
  const jetpackIndex = level.jetpacks.indexOf(jetpack);
  const jetpackBase = MEM.COIN_BASE + level.coins.length;
  const vm = createHostVM(jetpack.level);
  vm.tape[MEM.PLAYER_X] = localX(jetpack.level, jetpack.x);
  vm.tape[MEM.PLAYER_Y] = jetpack.y;

  tick(vm);

  assert.equal(vm.tape[MEM.JETPACK_COLLECTED], 1);
  assert.equal(vm.tape[jetpackBase + jetpackIndex], 1);
  assert.equal(vm.tape[MEM.JETPACK_FUEL], 150);

  tick(vm, { jetpack: 1 });
  tick(vm);
  tick(vm);
  tick(vm);

  assert.equal(vm.tape[MEM.JETPACK_ACTIVE], 1);
  assert.equal(vm.tape[MEM.JETPACK_FUEL], 149);

  tick(vm);
  tick(vm);
  tick(vm);
  tick(vm);

  assert.equal(vm.tape[MEM.JETPACK_ACTIVE], 1);
  assert.equal(vm.tape[MEM.JETPACK_FUEL], 148);
});

test("browser host observes closed door collision and open door entry", () => {
  const closed = createHostVM();
  closed.tape[MEM.PLAYER_X] = localX(1, level.door.x) - 1;
  closed.tape[MEM.PLAYER_Y] = level.door.y;
  tick(closed, { right: 1 });

  assert.equal(closed.tape[MEM.PLAYER_X], localX(1, level.door.x) - 1);
  assert.equal(closed.tape[MEM.PLAYER_SUB_X], 0);

  const open = createHostVM();
  open.tape[MEM.PLAYER_X] = localX(1, level.door.x) - 1;
  open.tape[MEM.PLAYER_Y] = level.door.y;
  open.tape[MEM.DOOR_OPEN] = 1;
  tick(open, { right: 1 });

  assert.equal(open.tape[MEM.PLAYER_X], localX(1, level.door.x) - 1);
  assert.equal(open.tape[MEM.PLAYER_SUB_X], 1);
});

test("browser host observes BF-owned enemy, death, win, and restart", () => {
  const enemySource = level.enemies[0];
  if (enemySource) {
    const enemy = createHostVM(enemySource.level);
    enemy.tape[MEM.PLAYER_X] = localX(enemySource.level, enemySource.x);
    enemy.tape[MEM.PLAYER_Y] = enemySource.y;
    tick(enemy);
    assert.equal(enemy.tape[MEM.GAME_DEAD], 1);
  }

  const win = createHostVM();
  const door = finalDoor();
  win.tape[MEM.CURRENT_LEVEL] = level.levelCount;
  win.tape[MEM.PLAYER_X] = localX(level.levelCount, door.x);
  win.tape[MEM.PLAYER_Y] = door.y;
  win.tape[MEM.DOOR_OPEN] = 1;
  tick(win);
  assert.equal(win.tape[MEM.GAME_WIN], 1);

  tick(win, { jump: 0, right: 0, left: 0, restart: 1 });
  assert.equal(win.tape[MEM.PLAYER_X], localX(1, level.playerStart.x));
  assert.equal(win.tape[MEM.PLAYER_Y], level.playerStart.y);
  assert.equal(win.tape[MEM.GAME_WIN], 0);
  assert.equal(win.tape[MEM.CURRENT_LEVEL], 1);
  assert.equal(win.tape[MEM.DOOR_OPEN], 0);
  assert.equal(win.tape[MEM.SCORE], 0);
});
