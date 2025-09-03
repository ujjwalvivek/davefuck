export const MEM = Object.freeze({
  TICK_REQUESTED: 0,
  TICK_DONE: 1,
  INPUT_LEFT: 33,
  INPUT_RIGHT: 32,
  INPUT_JUMP: 34,
  INPUT_RESTART: 35,
  PLAYER_X: 64,
  PLAYER_Y: 65,
  PLAYER_MOVE_COOLDOWN: 66,
  PLAYER_JUMP_PHASE: 67,
  PLAYER_JUMP_TIMER: 68,
  PLAYER_SUB_X: 69,
  PLAYER_SUB_Y: 70,
  PLAYER_STEP_DONE: 71,
  COLLISION_BLOCKED: 72,
  KEY_COLLECTED: 112,
  DOOR_OPEN: 113,
  ENEMY_X: 114,
  ENEMY_Y: 115,
  ENEMY_DIR: 116,
  ENEMY_TIMER: 117,
  ENEMY_STEP_DONE: 118,
  GAME_DEAD: 119,
  GAME_WIN: 120,
  GAME_STARTED: 121,
  PLAYER_FACING: 122,
  SCORE: 123,
  COIN_BASE: 128,
});

const OPS = new Set([">", "<", "+", "-", ".", ",", "[", "]"]);

export function compileBrainfuck(source) {
  const code = [];
  const stack = [];

  for (const char of source) {
    if (!OPS.has(char)) continue;

    if (char === "[") {
      stack.push(code.length);
      code.push({ op: "[", jump: -1 });
      continue;
    }

    if (char === "]") {
      const open = stack.pop();
      if (open === undefined) throw new Error("Unmatched closing bracket");
      code.push({ op: "]", jump: open });
      code[open].jump = code.length - 1;
      continue;
    }

    code.push({ op: char });
  }

  if (stack.length > 0) throw new Error("Unmatched opening bracket");
  return code;
}

export class BrainfuckVM {
  constructor(sourceOrCode, options = {}) {
    this.code =
      typeof sourceOrCode === "string" ? compileBrainfuck(sourceOrCode) : sourceOrCode;
    this.tape = options.tape ?? new Uint8Array(options.tapeSize ?? 512);
    this.pointer = 0;
    this.pc = 0;
    this.input = options.input ?? (() => 0);
    this.output = options.output ?? (() => {});
    this.halted = false;
  }

  step() {
    if (this.pc >= this.code.length) {
      this.halted = true;
      return false;
    }

    const instruction = this.code[this.pc];
    switch (instruction.op) {
      case ">":
        this.pointer += 1;
        if (this.pointer >= this.tape.length) throw new Error("Tape pointer moved past end");
        break;
      case "<":
        this.pointer -= 1;
        if (this.pointer < 0) throw new Error("Tape pointer moved before start");
        break;
      case "+":
        this.tape[this.pointer] = (this.tape[this.pointer] + 1) & 255;
        break;
      case "-":
        this.tape[this.pointer] = (this.tape[this.pointer] - 1) & 255;
        break;
      case ".":
        this.output(this.tape[this.pointer]);
        break;
      case ",":
        this.tape[this.pointer] = this.input() & 255;
        break;
      case "[":
        if (this.tape[this.pointer] === 0) this.pc = instruction.jump;
        break;
      case "]":
        if (this.tape[this.pointer] !== 0) this.pc = instruction.jump;
        break;
      default:
        throw new Error(`Unknown opcode ${instruction.op}`);
    }

    this.pc += 1;
    return true;
  }

  rewind() {
    this.pointer = 0;
    this.pc = 0;
    this.halted = false;
  }

  runUntil(predicate, maxSteps = 100000) {
    let steps = 0;
    while (!this.halted && !predicate(this) && steps < maxSteps) {
      this.step();
      steps += 1;
    }
    if (!predicate(this)) {
      throw new Error(`VM condition not reached within ${maxSteps} steps`);
    }
    return steps;
  }
}

async function startDevServer() {
  const fs = await import("node:fs");
  const http = await import("node:http");
  const path = await import("node:path");

  const root = process.cwd();
  const preferredPort = Number(process.env.PORT ?? 4173);
  const types = new Map([
    [".html", "text/html; charset=utf-8"],
    [".js", "text/javascript; charset=utf-8"],
    [".bf", "text/plain; charset=utf-8"],
    [".css", "text/css; charset=utf-8"],
  ]);

  function resolveRequest(url) {
    const pathname = decodeURIComponent(new URL(url, "http://localhost").pathname);
    const webAsset = new Set(["/runtime.js", "/vm.js"]);
    const relative =
      pathname === "/"
        ? "runners/web/index.html"
        : webAsset.has(pathname)
          ? `runners/web${pathname}`
          : pathname.slice(1);
    const resolved = path.resolve(root, relative);
    return resolved.startsWith(root) ? resolved : null;
  }

  const server = http.createServer((request, response) => {
    const file = resolveRequest(request.url);
    if (!file || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
      response.writeHead(404);
      response.end("not found");
      return;
    }

    response.writeHead(200, {
      "content-type": types.get(path.extname(file)) ?? "application/octet-stream",
    });
    fs.createReadStream(file).pipe(response);
  });

  function listen(port) {
    server.once("error", (error) => {
      if (error.code === "EADDRINUSE") {
        listen(port + 1);
        return;
      }
      throw error;
    });
    server.listen(port, () => console.log(`http://localhost:${port}`));
  }

  listen(preferredPort);
}

if (typeof process !== "undefined" && process.argv?.includes("--serve")) {
  await startDevServer();
}
