# Brainfuck Dave

A tiny one-screen platformer whose gameplay is handled by Brainfuck.

The player moves, jumps, collides with walls, collects a key, opens a door, avoids an enemy, dies, wins, and restarts because `rom/dave.bf` changes cells on a Brainfuck tape. The JavaScript host only runs the Brainfuck VM, copies input into tape cells, and renders the resulting tape state.

## Commands

```sh
# run the browser host at http://localhost:4173
npm run dev

# test the Brainfuck ROM
npm test

# report raw and gzip size for the web host, generated level metadata, and ROM
npm run size
```

## Controls

```txt
Arrow Left / Arrow Right  move
Arrow Up / Space          jump
R                         restart
```

## Tape Contract

The web host writes input cells before each tick:

```asm
0   TICK_REQUESTED
1   TICK_DONE
32  INPUT_RIGHT
33  INPUT_LEFT
34  INPUT_JUMP
35  INPUT_RESTART
```

The ROM updates gameplay cells:

```asm
64  PLAYER_X
65  PLAYER_Y
67  PLAYER_JUMP_PHASE
68  PLAYER_JUMP_TIMER
69  PLAYER_SUB_X
70  PLAYER_SUB_Y

112 KEY_COLLECTED
113 DOOR_OPEN
114 ENEMY_X
115 ENEMY_Y
116 ENEMY_DIR
117 ENEMY_TIMER
119 GAME_DEAD
120 GAME_WIN
121 GAME_STARTED
```

The renderer reads those cells.

**Note:** It does not implement movement, collision, enemy behavior, key logic, door logic, death, or win.

## What This Is

This release is the runnable Brainfuck ROM and web host.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
