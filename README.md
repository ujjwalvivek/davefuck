# Davefuck

Dangerous Dave written in Brainfuck that runs in the browser, with a ~60KB payload.

Anything happening in game is because `rom/dave.bf` changes cells on a Brainfuck tape. The JavaScript code is just the host for the ROM.

This release is the runnable Brainfuck ROM and web host.

## Controls

| Key                      | Action                  |
| ------------------------ | ----------------------- |
| Arrow Left / Arrow Right | move                    |
| Arrow Up                 | jump                    |
| R                        | restart                 |
| SPACE                    | shoot                   |
| LEFT ALT                 | jetpack enamble/disbale |

## Commands

```sh
npm run dev    # run the browser host at http://localhost:4173
npm test       # test the Brainfuck ROM
npm run size   # report raw and gzip size for the payload
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE).
