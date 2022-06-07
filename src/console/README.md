# Console add-on
This addon provides an interactive console for Minecraft process' output. this is useful to debug 
the game when it crashes multiple times, or simply if you can to track what's going on.

![PyPI - Version](https://img.shields.io/pypi/v/portablemc-console?style=flat-square) &nbsp;![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc-console?label=PyPI%20downloads&style=flat-square)

```console
pip install --user portablemc-console
```

## Usage
**This addon requires you to install the [prompt_toolkit](https://pypi.org/project/prompt-toolkit/) python 
library.**

This addon is enabled by default when launching the game with the `start` sub-command. To disable 
it and fall back to the default process' output, you can add the `--no-console` flag to the command
line. By default, when the game ends, you need to do Ctrl+C again to close the terminal, you
can disable it using the `--single-exit` flag in the command, this will cause your interactive
console to close with the game's process.

## Examples
```sh
portablemc start my_version               # Starts the game and open the interactive console. 
portablemc start --no-console my_version  # Starts the game and don't open the interactive console.
```

![interactive console screenshot](/doc/assets/console.png)

## Credits
- [PyPI page of prompt_toolkit](https://pypi.org/project/prompt-toolkit/)
