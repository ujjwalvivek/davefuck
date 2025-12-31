# Davefuck

![Echopoint SVG](https://echopoint.ujjwalvivek.com/svg/badges/custom?leftText=brainfuck&rightText=ROM&badgeColor=808000&textColor=ffffff)
![Echopoint SVG](https://echopoint.ujjwalvivek.com/svg/badges/custom?leftText=renderer&rightText=JS+|+RUST&badgeColor=804000&textColor=ffffff)
![Echopoint SVG](https://echopoint.ujjwalvivek.com/svg/badges/custom?leftText=60KB&rightText=WEB+PAYLOAD&badgeColor=400040&textColor=ffffff)

<img src="https://github.com/user-attachments/assets/2f5416b9-612f-4992-aef6-2e55ce0ff9e0" alt="davefuck_showcase" width="1000">

Dangerous Dave written in Brainfuck that runs in the browser, with a ~60KB payload.

Anything happening in game is because `rom/dave.bf` changes cells on a Brainfuck tape. The JavaScript code is just the host for the ROM.

This release is the runnable Brainfuck ROM and web host.

## Compiler & Native Runner

The compiler and the native runner have now been open-sourced. You can find them in the [davefuck-compiler](compiler) and [davefuck-native](runners/native) repositories.

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
