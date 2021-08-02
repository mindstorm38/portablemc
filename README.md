# Portable Minecraft Launcher
An easy-to-use portable Minecraft launcher in only one Python script!
This single-script launcher is still compatible with the official (Mojang) Minecraft Launcher stored
in `.minecraft` and use it. The goals are speed and reliability for all Minecraft versions in a 
stateless manner. You can now customize the launcher with addons.

![GitHub release (latest by date)](https://img.shields.io/github/v/release/mindstorm38/portablemc?label=stable&style=flat-square)&nbsp;&nbsp;![GitHub release (latest by date including pre-releases)](https://img.shields.io/github/v/release/mindstorm38/portablemc?include_prereleases&label=preview&style=flat-square)&nbsp;&nbsp;![GitHub all releases](https://img.shields.io/github/downloads/mindstorm38/portablemc/total?label=Github%20downloads&style=flat-square)&nbsp;&nbsp;![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc?label=PyPI%20downloads&style=flat-square)

### [Install now!](#installation) *[Fabric is now supported!](#fabric-support)*

![illustration](https://github.com/mindstorm38/portablemc/blob/master/doc/illustration.png?raw=true)

*This launcher is tested for Python 3.6, 3.7, 3.8, further testing using other versions are welcome.*

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
  - [Fabric support](#fabric-support)
  - [Better console](#better-console)
  - [Archives support](#archives-support)

# Installation
The launcher can be installed in several ways, including Python Package Index *(PyPI)* or manually using the 
single-file script. Before starting, please check if your Python version is valid for the launcher by doing `python -V`, 
the version must be greater or equal to 3.6.

## Install with PIP
The easiest way to install the launcher is to use the `pip` tool of your Python installation. On some linux distribution 
you might have to use `pip3` instead of `pip` in order to run it on Python 3. You can also use `python -m pip` you the
`pip` command is not in the path and the python executable is.

```sh
pip install --user portablemc
```

We advise you to keep `--user` because this allows to install the launcher locally, it is implicit if you are not an 
administrator and if you are, it allows not to modify other users' installation.

After that, you can try to show the launcher help message using `portablemc` in your terminal. If it fails, you must
ensure that the scripts directory is in your user path environment variable. On Windows you have to search for a
directory at `%appdata%/Python/Python3X/Scripts` and add it to the user's environment variable `Path`. On UNIX
systems this should work properly because the script is put in `~/.local/bin`.

# Single-file script
On each release, a single-file script is built and distributed on the [release page](https://github.com/mindstorm38/portablemc/releases).
This file has not to be installed, you can just run it using `python portablemc.py [...]`, on UNIX you can start the script
directly with `portablemc.py` because the file has a *[shebang](https://wikipedia.org/wiki/Shebang)*.

# Sub-commands
Arguments are split between multiple sub-commands. For example `<exec> <sub-command>`. You can use `-h` 
argument to display help *(also works for every sub-commands)*.

You may need to use `--main-dir <path>` if you want to change the main directory of the game. The main
directory stores libraries, assets, versions. **By default** the location
of this directory is OS-dependent, but always in your user's home directory, 
[check wiki for more information](https://minecraft.gamepedia.com/.minecraft).

You may also need `--work-dir <path>` to change the directory where your saves, resource packs and
all "user-specific" content is stored. This can be useful if you have a shared read-only main directory 
(`--main-dir`) and user-specific working directory (for example in `~/.minecraft`, by default it's the
locaton of your main directory). This launcher also stores the authentication credentials in this directory
(since launcher version 1.1.4).

The two arguments `--main-dir` and `--work-dir` may or may not be used by sub commands, then you can alias
the command and always set the main and work directory like you want.

## Start the game
The `<exec> start [arguments...] [version]` sub-command is used to prepare and launch the game. A lot
of arguments allow you to control how to game will behave. The only positional argument is the version - 
you can either specify a full version id (which you can get from the [search](#search-for-versions) 
sub-command), or a type of version to select the latest of this type (`release` (default) or `snapshot`).

### Authentication
Online mode is supported by this launcher, use the `-l <email_or_username>` (`--login`) argument to
log into your account *(login with a username is deprecated by Mojang)*. If your session is not
cached nor valid, the launcher will ask for the password.

You can now use the the `-m` (`--microsoft`) to authenticate a Microsoft account if you already had
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
You can change it by providing a path to the `java` binary with the `--jvm <path_to/bin/java>` argument. By default, the launcher starts the JVM with default arguments, 
these are the following and are the same as the Mojang launcher:

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

## Search for versions
The `<exec> search [-l] [version]` sub-command is used to search for versions. By default, this command
will search for official versions available to download, you can instead search for local versions
using the `-l` (`--local`) flag. The search string is optional, if not given all official or local
versions are displayed.

## Authentication sessions
Two subcommands allow you to store or logout of sessions: `<exec> login|logout <email_or_username>`.
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
The `<exec> addon list|dirs|show` sub-commands are used to list and show addons. The `addon dirs` subcommand is used
to list all directories where you can place the addons' folders.

# Addons
Officially supported addons can be found in the ['addons' directory](https://github.com/mindstorm38/portablemc/tree/master/addons).
To install addons you have to run `addon dirs` to get all directories where you can place addons.
To check if the addons are properly installed, you can use the ['addon list' sub-command](#addons).

## Fabric support
[**Download**](https://minhaskamal.github.io/DownGit/#/home?url=https://github.com/mindstorm38/portablemc/tree/master/addons/fabric)

Fabric is now supported through the addon `fabric`.

This add-on allows you to start Minecraft using FabricMC directly with the [start sub-command](#start-the-game), 
but instead of a standard version like `1.16.5` you must use the following pattern: `fabric:<mc-version>`.
Use `fabric:` to start fabric for latest release, `<mc-version>` can be a version type (`release` or `snapshot`), 
in this case the latest version of this type is selected.

For example, using the command `portablemc.py start fabric:1.16.5` will download and start the latest FabricMC mod loader for `1.16.5`.

You can also specify the loader version in addition using the following pattern: `fabric:<mc-version>:<loader-version>`.

***For now, mods must be installed manually in the standard `mods` directory, an additional command to install and 
manage mods was planed but this is not possible for now due to complex APIs and mods management by Fabric.***

![fabric animation](https://github.com/mindstorm38/portablemc/blob/master/doc/fabricmc.gif?raw=true)

## Better console
[**Download**](https://downgit.github.io/#/home?url=https://github.com/mindstorm38/portablemc/tree/master/addons/console)

An addon named `console` can be used to display the Minecraft process' console, this is useful to debug the game when
it crashes multiple times, or simply if you can to track what's going on.
An overview of the console can be seen in the animated image in the fabric section just above, it provides a blue header
section with summary of the running session and Minecraft version and lines are printed bellow it, you can then navigate
the output buffer.

## Archives support
[**Download**](https://downgit.github.io/#/home?url=https://github.com/mindstorm38/portablemc/tree/master/addons/archives)

An addon named `archives` allows you to launch archived Minecraft versions.
This addon extends the [start sub-command](#start-the-game) and you can use `arc:` prefix, for exemple `start arc:a1.1.1`
will download, install and run the Alpha 1.1.1 version from the archives. This addon also extends the `search` subcommand
with an argument `--archives` (`-a`) to search versions in the archives.

***This addon is based on all the work done by the [Omniarchive community](https://omniarchive.net/).***
All types of archived versions are supported:
- [Pre-Classic (Rubydung)](https://archive.org/details/Minecraft-JE-Pre-Classic)
- [Classic](https://archive.org/details/Minecraft-JE-Classic)
- [Indev](https://archive.org/details/Minecraft-JE-Indev)
- [Infdev](https://archive.org/details/Minecraft-JE-Infdev)
- [Alpha](https://archive.org/details/Minecraft-JE-Alpha)
- [Beta](https://archive.org/details/Minecraft-JE-Beta)
