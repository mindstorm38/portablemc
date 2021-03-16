# Portable Minecraft Launcher
An easy-to-use portable Minecraft launcher in only one Python script!
This single-script launcher is still compatible with the official (Mojang) Minecraft Launcher stored
in `.minecraft` and use it.
You can now customize the launcher with addons.

![illustration](https://github.com/mindstorm38/portablemc/blob/master/illustration.png?raw=true)

*This launcher is tested for Python 3.8 & 3.6, further testing using other versions are welcome.*

# Table of contents
- [Sub-commands](#sub-commands)
  - [Start the game](#start-the-game)
    - [Authentication](#authentication)
    - [Offline mode](#offline-mode)
    - [Working directory](#working-directory)
    - [Custom JVM](#custom-jvm)
    - [Auto connect to a server](#auto-connect-to-a-server)
    - [Miscellaneous](#miscellaneous)

# Sub-commands
Arguments are split between multiple sub-command. For example `<exec> <sub-command>` will start the latest
release with default parameters. You can use `-h` argument to display help *(also work for sub-commands)*.

You may need to use `--main-dir <path>` if you want to change the main directory of the game. The main
directory stores libraries, assets, versions and this launcher's credentials. **By default** the location
of this directory is OS-dependent, but always in your user's home directory, 
[check wiki for more information](https://minecraft-fr.gamepedia.com/.minecraft).

**In this example**, `<exec>` must be replaced by any command that 
launch the script, for example `python3 portablemc.py`.

**Note that** this script have a *[shebang](https://fr.wikipedia.org/wiki/Shebang)*, this can be
useful to launch the script on unix OS *(you must have executable permission)*.

## Start the game
The `<exec> start` sub-command is used to prepare and launch the game. A lot of arguments allows you
to control how to game will behave.

### Authentication
Online mode is supported by this launcher, use the `-l <email_or_username>` (`--login`) argument to
log into your account *(login with a username is now deprecated by Mojang)*. If your session is not
cached or no longer valid, the launcher will ask for the password.

You can disable the session caching using the flag argument `-t` (`--temp-login`), using this argument,
if your session is nor cached or valid you will be asked for the password for every launch.

**Note that** your password is not saved! Only the token is saved (the official launcher also do that)
in the file `portablemc_tokens` in the main directory (an argument may change this location in the
future).

### Offline mode
If you need fake offline accounts you can use `-u <username>` (`--username`) defines the username and/or
`-i <uuid>` (`--uuid`) to define your player's [UUID](https://fr.wikipedia.org/wiki/Universally_unique_identifier).

If you omit the UUID, a random one is choosen. If you omit the username, the first 8 characters of the UUID
are used for it. **These two arguments are overwritten by the `-l` (`--login`) argument.

### Working directory
You can use the argument `--work-dir <path>` to change the directory where your saves, resource packs and
all "user-specific" content. This can be useful if you have a shared read-only main directory 
(`--main-dir`) and user-specific working directory (for example in `~/.minecraft`).

When starting the game, the binaries (`.DLL`, `.SO` for exemple) are temporary copied to the directory
`<main_dir>/bin`, but you can tell the launcher to copy these binaries into your working directory using
the `--work-dir-bin` argument. This may be useful if you don't have permissions on the main directory.

### Custom JVM
The Java Virtual Machine is used to run the game, by udefault the launcher use the `java` executable. You
can change it using `--jvm <path>` argument. By default, some JVM arguments are also passed, these arguments
are the following and were copied from the officiel launcher:

```
-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
```

You can change these arguments using the `--jvm-args <args>`.

### Auto connect to a server
Since Minecraft 1.6 *(at least, need further tests to confirm)* we can start the game and automatically
connect to a server. To do that you can use `-s <addr>` (`--server`) for the server address 
(e.g. `mc.hypixel.net`) and the `-p` (`--server-port`) to specify its port, by default to 25565.


### Miscellaneous
With `--dry`, the game is prepared but not started.

With `--demo` you can enable the [demo mode](https://minecraft.gamepedia.com/Demo_mode) of the game.  

With `--resol <width>x<height>` you can change the resolution of the game window.

With `--no-better-logging` flag you can disable the better logging configuration used by the launcher
to avoid raw XML logging in the terminal.

The two arguments `--disable-mp` (mp: multiplayer), `--disable-chat` are obvious *(since 1.16)*.

## Search
The `<exec> search <version>` sub-command is used to search for versions 