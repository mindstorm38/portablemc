# Portable Minecraft Launcher
An easy-to-use portable Minecraft launcher in only one Python script!
This single-script launcher is compatible with the directory structure of the official Minecraft Launcher.
It aims to be fast and reliable for all Minecraft versions in a stateless manner and support addons.

![GitHub release (latest by date)](https://img.shields.io/github/v/release/mindstorm38/portablemc?label=stable&style=flat-square) &nbsp;![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/mindstorm38/portablemc?include_prereleases&label=preview&style=flat-square) &nbsp;![GitHub all releases](https://img.shields.io/github/downloads/mindstorm38/portablemc/total?label=Github%20downloads&style=flat-square) &nbsp;![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc?label=PyPI%20downloads&style=flat-square) &nbsp;![Release cycle](https://img.shields.io/badge/Release%20cycle-2%2Fmonth-yellow?style=flat-square)

### [Install now!](#installation)

#### ***[Fabric](/addons/fabric/README.md) and [Forge](/addons/forge/README.md) are now supported!***

**The launcher is safe to Log4j exploit since v2.2.0, if you are running an older please update or read the
following issue for a temporary fix: [#52](https://github.com/mindstorm38/portablemc/issues/52).**

![illustration](doc/assets/illustration.png)

*This launcher is tested for Python 3.6, 3.7, 3.8, 3.9, further testing using other versions are welcome.*

# Table of contents
- [Installation](#installation)
  - [Install with PIP](#install-with-pip)
  - [Single-file script](#single-file-script)
- [Sub-commands](#sub-commands)
  - [Start the game](#start-the-game)
    - [Authentication](#authentication)
    - [Offline mode](#offline-mode)
    - [Custom JVM](#custom-jvm)
    - [Auto connect to a server](#auto-connect-to-a-server)
    - [Miscellaneous](#miscellaneous)
  - [Search for versions](#search-for-versions)
  - [Authentication sessions](#authentication-sessions)
  - [Addon sub-command](#addon-sub-command)
- [Addons](#addons)
  - [Fabric ⇗](/addons/fabric/README.md)
  - [Forge ⇗](/addons/forge/README.md)
  - [Console ⇗](/addons/console/README.md)
  - [Archives ⇗](/addons/archives/README.md)
  - [Modrinth ⇗](/addons/modrinth/README.md)
- [Contributors](#contributors)
- [API Documentation ⇗](doc/API.md)
- [Addon API Documentation ⇗](doc/ADDON.md)

# Installation
The launcher can be installed in several ways, using Python Package Index *(PyPI)* or manually running the 
single-file script. Before starting, please check if your Python version is valid for the launcher by doing 
`python -V`, the version must be greater or equal to 3.6 according to [semver specification](https://semver.org/),
this launcher's version is also following the semver specification.

## Install with PIP
The easiest way to install the launcher is to use the `pip` tool of your Python installation. On some linux distribution 
you might have to use `pip3` instead of `pip` in order to run it on Python 3. You can also use `python -m pip` if the
`pip` command is not in the path and the python executable is.

```sh
pip install --user portablemc
```

We advise you to keep `--user` because this allows to install the launcher for your current user only, it is implicit 
if you are not an administrator and if you are, it allows not to modify other users' installations.

After that, you can try to show the launcher help message using `portablemc` in your terminal. If it fails, you must
ensure that the scripts directory is in your user path environment variable. On Windows you have to search for a
directory at `%appdata%/Python/Python3X/Scripts` and add it to the user's environment variable `Path`. On UNIX
systems this should work properly because the script is put in `~/.local/bin`.

## Single-file script
On each release, a single-file script is built and distributed on the [release page](https://github.com/mindstorm38/portablemc/releases).
This file has not to be installed, you can just run it using `python portablemc.py [...]`, on UNIX you can start the script
directly with `portablemc.py` because the file has a *[shebang](https://wikipedia.org/wiki/Shebang)*.

# Sub-commands
Arguments are split between multiple sub-commands. For example `portablemc <sub-command>`. You can use `-h` 
argument to display help *(also works for every sub-commands)*.

You may need to use `--main-dir <path>` if you want to change the main directory of the game. The main
directory stores libraries, assets, versions. **By default** the location
of this directory is OS-dependent, but always in your user's home directory, 
[check wiki for more information](https://minecraft.gamepedia.com/.minecraft).

You may also need `--work-dir <path>` to change the directory where your saves, resource packs and
all "user-specific" content is stored. This can be useful if you have a shared read-only main directory 
(`--main-dir`) and user-specific working directory (for example in `~/.minecraft`, by default it's the
location of your main directory). This launcher also stores the authentication credentials in this directory
(since launcher version 1.1.4).

The two arguments `--main-dir` and `--work-dir` may or may not be used by sub commands, then you can alias
the command and always set the main and work directory like you want.

An advanced argument `--timeout <seconds>` can be used to set a global timeout value that can be freely used
by the launcher or addons. You can use the special value 0 to force using local caches, is supported. For
now, it's used by the launcher only for the version manifest fetching as it is now locally cached.

## Start the game
The `portablemc start [arguments...] [version]` sub-command is used to prepare and launch the game. A lot
of arguments allow you to control how to game will behave. The only positional argument is the version, 
which can be either a full version id (which you can get from the [search](#search-for-versions) 
sub-command), or a type of version to select the latest of this type (`release` (default) or `snapshot`),
if you omit the version argument, it's equivalent to `release`.

### Authentication
Online mode is supported by this launcher, use the `-l <email_or_username>` (`--login`) argument to
log into your account *(login with a username is deprecated by Mojang)*. If your session is not
cached nor valid, the launcher will ask for the password.

You can use the the `-m` (`--microsoft`) to authenticate a Microsoft account if you already had
migrated your account. In this case the launcher will open a page in your web browser with the
Microsoft login page.

You can disable session caching using the argument `-t` (`--temp-login`). If your session is 
not cached nor valid, you will be asked for the password on every launch.

You can also use `--anonymise` in order to hide most of your email when printing it to the terminal. For example,
`foo.bar@gmail.com` will become `f*****r@g***l.com`, this is useful to avoid leaking it when recording or streaming.
However, if you use this, make sure that you either use an alias or a variable with the `-l` argument, for exemple
`-l $PMC_LOGIN`.

**[Check below](#authentication-sessions) for more information about authentication sessions.**

### Offline mode
If you need fake offline accounts you can use `-u <username>` (`--username`) to define the username and/or
`-i <uuid>` (`--uuid`) to define your player's [UUID](https://wikipedia.org/wiki/Universally_unique_identifier).

If you omit the UUID, a random one is chosen. If you omit the username, the first 8 characters of the UUID
are used for it. **These two arguments are overwritten by the `-l` (`--login`) argument**.

### Custom JVM
The launcher uses Java Virtual Machine to run the game, by default the launcher downloads and uses the official JVM 
[distributed by Mojang](https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json)
which is adapted to the running version. The JVM is installed in a sub-directory called `jvm` inside the main directory. 
You can change it by providing a path to the `java` binary with the `--jvm <path_to/bin/java>` argument. By default, the
launcher starts the JVM with default arguments, these are the following and are the same as the Mojang launcher:

```
-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
```

You can change these arguments using the `--jvm-args <args>`, **please always quote your set of arguments**, this set must
be one argument for PMC. For example `--jvm-args "-Xmx2G -XX:+UnlockExperimentalVMOptions"`.

### Auto connect to a server
Since Minecraft 1.6 we can start the game and automatically connect to a server. To do that you can use 
`-s <addr>` (`--server`) for the server address (e.g. `mc.hypixel.net`) and the `-p` (`--server-port`) 
to specify its port, by default to 25565.

### Miscellaneous
With `--dry`, the game is prepared but not started.

With `--demo` you can enable the [demo mode](https://minecraft.gamepedia.com/Demo_mode) of the game.  

With `--resol <width>x<height>` you can change the resolution of the game window.

With `--no-better-logging` flag you can disable the better logging configuration used by the launcher
to avoid raw XML logging in the terminal.

The two arguments `--disable-mp` (mp: multiplayer), `--disable-chat` are obvious *(since 1.16)*.

## Search for versions
The `portablemc search [-l] [version]` sub-command is used to search for versions. By default, this command
will search for official versions available to download, you can instead search for local versions
using the `-l` (`--local`) flag. The search string is optional, if not given all official or local
versions are displayed.

## Authentication sessions
Two subcommands allow you to store or logout of sessions: `portablemc login|logout <email_or_username>`.
These subcommands don't prevent you from using the `-l` (`--login`) argument when starting the game,
these are just here to manage the session storage.

A new argument `-m` (`--microsoft`) is available for both subcommands since `1.1.4` for migrated 
Microsoft accounts.
The launcher will open the Microsoft login page (with your email pre-typed in) in your web browser 
and wait until validated. 

**Your password is not saved!** Only a token is saved (the official launcher also does that)
in the file `portablemc_auth.json` in the working directory. In older version of the launcher
(`< 1.1.4`), this file was `portablemc_tokens` in the main directory, the migration from the old
file is automatic and irreversible (the old file is deleted).

## Addon sub-command
The `portablemc addon list|dirs|show` sub-commands are used to list and show addons. The `addon dirs` subcommand is used
to list all directories where you can place the addons' folders.

# Addons
Officially supported addons can be found in the ['addons' directory](/addons).
To install addons you have to run `addon dirs` to get all directories where you can place addons.
To check if the addons are properly installed, you can use the ['addon list' sub-command](#addons).

# Contributors
This launcher would not be as functional without the contributors, and in particular the following for their bug reports
and suggestions to make the launcher better: [GoodDay360](https://github.com/GoodDay360) and [Ristovski](https://github.com/Ristovski).

There must be a lot of hidden issues, if you want to contribute you just have to install and test the launcher, and
report every issue you encounter, do not hesitate!
