# Archives add-on
The archives addon allows you to install and run old archived versions that are not officially
listed by Mojang. It is backed by the [omniarchive](https://omniarchive.net/) work.

[**Download (1.1.0)**](https://downgit.github.io/#/home?url=https://github.com/mindstorm38/portablemc/tree/master/addons/archives)

## Usage
This add-on extends the syntax accepted by the [start](/README.md#start-the-game) sub-command, by 
prepending the version with `arc:`. Every version starting with this prefix will be resolved from
archive repositories hosted on [archives.org](https://archive.org). This addon also add a `-a` 
(`--archives`) flag to the [search](/README.md#search-for-versions) sub-command. You can use it to
list all archived versions or search for some specific ones before actually trying to run them.

The following repositories are used to resolve your versions:
- [Pre-Classic (Rubydung)](https://archive.org/details/Minecraft-JE-Pre-Classic)
- [Classic](https://archive.org/details/Minecraft-JE-Classic)
- [Indev](https://archive.org/details/Minecraft-JE-Indev)
- [Infdev](https://archive.org/details/Minecraft-JE-Infdev)
- [Alpha](https://archive.org/details/Minecraft-JE-Alpha)
- [Beta](https://archive.org/details/Minecraft-JE-Beta)

## Examples
```sh
portablemc search -a                # List all archived versions.
portablemc search -a a1.2.0         # List all archived versions that contains the string 'a1.2.0'.
portablemc start arc:a1.2.0         # Start the archived version of alpha 1.2.0.
portablemc start --dry arc:a1.2.0   # Install the archived version of alpha 1.2.0 if it's not already the case.
```

## Credits
- [Omniarchive community](https://omniarchive.net/)
