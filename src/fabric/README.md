# Fabric add-on
The fabric add-on allows you to install and run Minecraft with fabric mod loader in a single command 
line!

![PyPI - Version](https://img.shields.io/pypi/v/portablemc-fabric?label=PyPI%20version&style=flat-square) &nbsp;![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc-fabric?label=PyPI%20downloads&style=flat-square)

```console
pip install --user portablemc-fabric
```

## Usage
This add-on extends the syntax accepted by the [start](/README.md#start-the-game) sub-command, by 
prepending the version with `fabric:`. Almost all releases since 1.14 are supported by fabric,
you can find more information on [fabric website](https://fabricmc.net/develop/), note the snapshots
are currently not supported by this addon, but this could be the case in the future because fabric
provides support for them. You can also use version aliases like `release` or equivalent empty version 
(just `fabric:`). This addon also provides a way of specifying the loader version, you just have to 
add `:<loader_version>` after the game version (the game version is still allowed to be aliases 
or empty, the following syntax is valid: `fabric::<loader_version>`).

This addon requires external HTTP accesses if:
- the game version is an alias.
- if the loader version is unspecified.
- if the specified version is not installed.

## Examples
```sh
portablemc start fabric:                # Start latest fabric loader version for latest release
portablemc start fabric:release         # Same as above
portablemc start fabric:1.19            # Start latest fabric loader version for 1.19
portablemc start fabric:1.19:0.14.8     # Start fabric loader 0.14.8 for game version 1.19
portablemc start fabric::0.14.8         # Start fabric loader 0.14.8 for the latest release
portablemc start --dry fabric:          # Install (and exit) the latest fabric loader version for latest release
```

![fabric animation](/doc/assets/fabricmc.gif)

## Credits
- [Fabric Website](https://fabricmc.net/)
