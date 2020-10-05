# Portable Minecraft Launcher
An easy to use portable Minecraft launcher in only one Python script !
This single-script launcher is still compatible with the official (Mojang) Minecraft Launcher stored in `.minecraft` and use it.

***[Mojang authentication now available !](#mojang-authentication)***

![illustration](https://github.com/mindstorm38/portablemc/blob/master/illustration.png?raw=true)

***This launcher is tester for Python 3.8 & 3.6, further testing using other versions are welcome.***

Once you have the script, you can launch it using python (e.g `python portablemc.py`).

# Arguments
The launcher support various arguments that make it really useful and faster than the official launcher
to test the game in offline mode *(custom username and UUID)*, or demo mode for example.

*You can read the complete help message using `-h` argument.*

***[Usage examples](#examples)***

### Mojang authentication
Do you want to authenticate using your Mojang account ?

It's now possible using `-l` *(`--login`)* followed by your email or username (for legacy account).
You will be asked for the password once the launcher is start. *If you don't want cache the session,
you can use `-t` (`--temp-login`) flag.*

> Session are stored in a separated file from official launcher *(`.minecraft/portablemc_tokens`)*,
note that no trace of your password remain in this file, so don't worry about using this !

> These arguments override arguments for offline username and UUID.

Your session is cached and you want to invalidate it ? Use `--logout` followed by your email or username.
This do not start the game.

### Minecraft version
By default the launcher starts the latest release version, to change this, you can use the `-v` *(`--version`)* followed by the
version name, or `snapshot` to target the latest snapshot, `release` does the same for latest release.

Using the `-s` *(`--search`)* flag you can tell this launcher to only search for all versions prefixed by the specified version of `-v` argument,
this stop the application just after searching. Exit codes: `15` if no version was found, else `0`.

> Note that latest version of Java may not work for old versions of Minecraft.

### Username and UUID (manual offline mode)
By default, a random player [UUID](https://fr.wikipedia.org/wiki/Universally_unique_identifier) is used, and the username is
extracted from the first part of the UUID's represention *(for a `110e8400-e29b-11d4-a716-446655440000` uuid, the username will be `110e8400`)*.

You can use `-u` *(`--username`)* followed by the username and `-i` *(`--uuid`)* with your user UUID.

> Note that even if you have set another UUID, the username will be the same as default (with extracted part from default UUID).

### Main & working directory
You can now configure directories used for game to work. These directories are:
- `-md` *(`--main-dir`)*: this directory store libraries, assets, versions, binaries (at runtime) and launcher cache *(default values [here](https://minecraft-fr.gamepedia.com/.minecraft))* 
- `-wd` *(`--work-dir`)*: this directory store game files like save, resource packs or logs *(if not specified, it is the same as main directory)*.

### Demo mode
Demo mode is a mostly unknown feature that allows to start the game with a restricted play duration, it is disabled by default.
Use `--demo` to enable.

### Window resolution
You can set the default window resolution *(does not affect the game if already in fullscreen mode)* by using `--resol` followed by
`<width>x<height>`, `width` and `height` are positive integers.

### No start mode
By using `--nostart` flag, you force the launcher to download all requirements to the game, but does not start it.

### Custom Java executable
By default the launcher use the `javaw` executable to launch Minecraft, if you want to
change this executable, use the `--java` argument followed by the executable.

# Examples
```
python portablemc.py                            Start latest Minecraft version using offline mode and random username and UUID
python portablemc.py -l <email|username>        Start latest Minecraft version using mojang authentication for specific email or username (legacy)
python portablemc.py -tl <email|username>       Same as previous command, but do not cache the session (you need to re-enter password on each launch)
python portablemc.py --nostart                  Download all components of the latest Minecraft version but do not start the game
python portablemc.py --logout <email|username>  Logout from a session
python portablemc.py -u OfflineTest -v 1.15     Start 1.15 Minecraft version in offline mode with a username 'OfflineTest' and random UUID
python portablemc.py -sv 1.7                    Search for all versions starting with "1.7"
```
