# Forge add-on
The forge add-on allows you to install and run Minecraft with forge mod loader in a single command 
line!

![PyPI - Version](https://img.shields.io/pypi/v/portablemc-forge?label=PyPI%20version&style=flat-square) &nbsp;![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc-forge?label=PyPI%20downloads&style=flat-square)

```console
pip install --user portablemc-forge
```

## Usage
This add-on extends the syntax accepted by the [start](/README.md#start-the-game) sub-command, by 
prepending the version with `forge:`. Almost all releases are supported by forge, the latest 
releases are often supported, if not please refer to forge website. You can also append either
`-recommended` or `-latest` to the version to take the corresponding version according to the
forge public information, this is reflecting the "Download Latest" and "Download Recommended" on
the forge website. You can also use version aliases like `release` or equivalent empty version 
(just `forge:`). You can also give the exact forge version like `1.18.1-39.0.7`, in such cases,
no HTTP request is made if the version is already installed.

*Note that this add-on uses the same JVM used to start the game (see `--jvm` argument).*

## Examples
```sh
portablemc start forge:               # Start recommended forge version for latest release
portablemc start forge:release        # Same as above
portablemc start forge:1.18.1         # Start recommended forge for 1.18.1
portablemc start forge:1.18.1-39.0.7  # Start the exact forge version 1.18.1-39.0.7
portablemc start --dry forge:         # Install (and exit) recommended forge version for latest release
```

## Credits
- [Forge Website](https://files.minecraftforge.net/net/minecraftforge/forge/)
- Consider supporting [LexManos](https://www.patreon.com/LexManos/)
