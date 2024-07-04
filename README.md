# Portable Minecraft Launcher
A fast, reliable and cross-platform command-line Minecraft launcher and API for developers.
Including fast and easy installation of common mod loaders such as Fabric, Forge, NeoForge and Quilt.
This launcher is compatible with the standard Minecraft directories. 

[![PyPI - Version](https://img.shields.io/pypi/v/portablemc?label=PyPI%20version&style=flat-square)![PyPI - Downloads](https://img.shields.io/pypi/dm/portablemc?label=PyPI%20downloads&style=flat-square)](https://pypi.org/project/portablemc/)

![illustration](https://github.com/mindstorm38/portablemc/blob/main/doc/illustration.png)

*This launcher is tested for Python 3.8, 3.9, 3.10, 3.11, 3.12.*

## Table of contents
- [Installation](#installation)
  - [With pip](#with-pip)
  - [With Arch Linux](#with-arch-linux)
- [Commands](#commands)
  - [Start Minecraft](#start-minecraft)
    - [Authentication](#authentication)
    - [Username and UUID](#username-and-uuid)
    - [Custom JVM](#custom-jvm)
    - [Server auto-connect](#server-auto-connect)
    - [LWJGL version and ARM support](#lwjgl-version-and-arm-support)
    - [Fix unsupported systems](#fix-unsupported-systems)
    - [Miscellaneous](#miscellaneous)
  - [Search for versions](#search-for-versions)
  - [Authentication sessions](#authentication-sessions)
  - [Shell completion](#shell-completion)
- [Offline support](#offline-support)
- [Certifi support](#certifi-support)
- [Contribute](#contribute)
  - [Setup environment](#setup-environment)
  - [Contributors](#contributors)
  - [Sponsors](#sponsors)
- [API Documentation (v4.3) ⇗](https://github.com/mindstorm38/portablemc/blob/main/doc/API.md)

## Installation

### With pip

This launcher can be installed using `pip`.  On some linux distribution you might have to 
use `pip3` instead of `pip` in order to run it on Python 3. You can also use 
`python -m pip` if the `pip` command is not in the path and the python executable is.

```sh
pip install --user portablemc[certifi]
```

After that, you can try to show the launcher help message using `portablemc` in your 
terminal. If it fails, you should check that the scripts directory is in your user path
environment variable. On Windows you have to search for a directory at 
`%appdata%/Python/Python3X/Scripts` and add it to the user's environment variable `Path`. 
On UNIX systems it's `~/.local/bin`.

You can opt-out from the `certifi` optional feature if you don't want to depend on it,
learn more in the [Certifi support](#certifi-support) section.

> [!TIP]
> It's recommended to keep `--user` because this installs the launcher for your current 
> user only and does not pollute other's environments, it is implicit if you are not an 
> administrator and if you are, it allows not to modify other users' installations.

### With Arch Linux

For Arch Linux users, the package is available as `portablemc` in the 
[AUR](https://aur.archlinux.org/packages/portablemc).

*This is currently maintained by Maks Jopek, Thanks!*

## Commands
Arguments are split between multiple commands. 
For example `portablemc [global-args] <cmd> [args]`. 
You can use `-h` argument to display help *(also works for every command)*.

By default the launcher will run any command from the OS standard `.minecraft` directory 
([check wiki for more information](https://minecraft.wiki/w/.minecraft)). You can
change this directory using `--main-dir <path>` global argument.

You may also need `--work-dir <path>` to change the directory where your saves, resource 
packs and all "user-specific" content is stored. This can be useful if you have a shared 
read-only main directory (`--main-dir`) and user-specific working directory (for example 
in `.minecraft`, by default it's the location of your main directory). The launcher also
stores cached version manifest and authentication database in the working directory.

The two arguments `--main-dir` and `--work-dir` may or may not be used by commands, 
but they are always valid to use, allowing you to define command aliases for running
PortableMC.

Another argument, `--timeout <seconds>` can be used to set a global timeout value that 
will be used for all network connections.

The general output format of the launcher can be changed using the `--output <mode>` with
one of the following modes:
- `human`: Human readable output, translated messages, formatted tables and tasks, 
  default if stdout if not a TTY.
- `human-color`: Same as `human` but with some color where relevant, like tasks states
  and game logs, default if stdout is a TTY.
- `machine`: Machine readable output, with one light per state change.

The verbosity of the launcher can be adjusted if you encounter issues, using multiple 
`-v` arguments (usually `-v` through `-vvv`). It's very useful to maintainers when fixing 
issues.

### Start Minecraft
The first thing you may want to do is install and start Minecraft, to do so you can use
the `portablemc start [args] [version]` command. This command will install every component
needed by the version before launching it. If you provide no version, the latest release
is started, but you can specify a version to launch, or a version alias: `release`
or `snapshot` for the latest version of their type.

In addition to Mojang's versions, the launcher natively supports common mod
loaders: 
[Fabric](https://fabricmc.net/), 
[Forge](https://minecraftforge.net/), 
[NeoForge](https://neoforged.net/), 
[LegacyFabric](https://legacyfabric.net/) and 
[Quilt](https://quiltmc.org/). 
To start such versions, you can prefix the version with either `fabric:`, `forge:`, 
`neoforge:`, `legacyfabric:` or `quilt:` (or `standard:` to explicitly choose a vanilla 
version). Depending on the mod loader, the version you put after the colon is different:
- For Fabric, LegacyFabric and Quilt, you can directly specify the vanilla version, 
  optionally followed by `:<loader_version>`. Note that legacy fabric start 1.13.2
  by default and does not support more recent version as it's not the goal.
- For Forge and NeoForge, you can put either a vanilla game version, optionally followed
  by `-<loader_version>`. Forge also supports `-latest` and `-recommended`, but NeoForge
  will always take the latest loader.

*You can search for versions using the [search command](#search-for-versions).*

```sh
# Start latest release
portablemc start
portablemc start release
# Start latest snapshot
portablemc start snapshot
# Start 1.20.1
portablemc start 1.20.1
# Start latest Fabric/Quilt/Forge version
portablemc start fabric:
portablemc start quilt:
portablemc start forge:
portablemc start neoforge:
# Start Fabric for 1.20.1
portablemc start fabric:1.20.1
# Start Fabric for 1.20.1 with loader 0.11.2
portablemc start fabric:1.20.1:0.11.2
# Start latest or recommended Forge for 1.20.1
portablemc start forge:1.20.1-latest
portablemc start forge:1.20.1-recommended
# Start Forge for 1.20.1 with loader 46.0.14
portablemc start forge:1.20-46.0.14
# Start NeoForge for 1.20.1
portablemc start neoforge:1.20.1
```

#### Authentication
Online mode is supported by this launcher, use the `-l <email_or_username>` (`--login`)
argument to log into your account *(login with a username is deprecated by Mojang)*. 
If your session is not cached or no longer valid, the launcher will ask for the 
password or open the Microsoft connection page.

**By default**, this will authenticate you using the Microsoft authentication services,
although you can change that using the `--auth-service` argument, for example with
`yggdrasil` if you need to log into an old Mojang account (being phased out by Mojang).

If you want to be asked for password on each authentication, you can use `-t`
(`--temp-login`). This has no effect if the session is already cached before that.

You can also use `--auth-anonymize` in order to hide most of your email when printing 
it to the terminal. For example, `foo.bar@gmail.com` will become `f*****r@g***l.com`,
this is useful to avoid leaking it when recording or streaming.
However, if you use this, make sure that you either use an alias or a variable with the
`-l` argument, for exemple `-l $PMC_LOGIN`.

*[Check below](#authentication-sessions) for more information about authentication 
sessions.*

#### Username and UUID
If you need fake offline accounts you can use `-u <username>` (`--username`) to define the
username and/or `-i <uuid>` (`--uuid`) to define your player's 
[UUID](https://wikipedia.org/wiki/Universally_unique_identifier).

If you omit the UUID, a random one is chosen. If you omit the username, the first 8 
characters of the UUID are used for it. 
**These two arguments are overwritten by the `-l` (`--login`) argument**.

#### Custom JVM
The launcher uses Java Virtual Machine to run the game, by default the launcher downloads
and uses the official JVM distributed by Mojang which is compatible with the game version. 
The JVM is installed in a sub-directory called `jvm` inside the main directory. 
You can change it by providing a path to the `java` binary with the
`--jvm <path_to/bin/java>` argument. By default, the launcher starts the JVM with default
arguments, these are the following and are the same as the Mojang launcher:

```
-Xmx2G -XX:+UnlockExperimentalVMOptions -XX:+UseG1GC -XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M
```

You can change these arguments using the `--jvm-args=<args>`, **please always quote your
set of arguments**, this set must be one argument for PMC. For example 
`portablemc start "--jvm-args=-Xmx2G -XX:+UnlockExperimentalVMOptions"`.

#### Server auto-connect
Since Minecraft 1.6 we can start the game and automatically connect to a server. 
To do so you can use `-s <addr>` (`--server`) for the server address 
(e.g. `mc.hypixel.net`) and the `-p` (`--server-port`) to specify the port, 
defaults to 25565.

*Modern releases use the quick play arguments rather than arguments specified above, the
behavior remains the same, singleplayer and realm are not yet supported by the launcher.*

#### LWJGL version and ARM support
With `--lwjgl VERSION` you can update the LWJGL version used when starting the game. This 
can be used to support ARM architectures, but this may only work with modern versions 
which are already using LWJGL 3. This argument works by dynamically rewriting the 
version's metadata, the new metadata is dumped in the version directory.

Using these versions on ARM is unstable and can show you an error with `GLXBadFBConfig`,
in such cases you should export the following environment variable 
`export MESA_GL_VERSION_OVERRIDE=4.5` 
([more info here](https://forum.winehq.org/viewtopic.php?f=8&t=34889)).

In case with the above you still get an `error: GLSL 1.50 is not supported` you may also 
try `export MESA_GLSL_VERSION_OVERRIDE=150`.

#### Fix unsupported systems
Some Mojang-provided natives (.so, .dll, .dylib) might not be compatible with your system.
To mitigate that, the launcher provides two arguments, `--exclude-lib` and `--include-bin`
that can be provided multiples times each.

With `--exclude-lib <artifact>[:[<version>][:<classifier>]]` you can exclude libraries 
(.jar) from the game's classpath (and so of the downloads). If a classifier is given, it
will match libs' classifiers that starts with itself, for example `lwjgl-glfw::natives`
will match the library `lwjgl-glfw:3.3.1:natives-windows-x86`.

With `--include-bin <bin-file>` you can dynamically include binary natives (.so, .dll,
.dylib) to the runtime's bin directory (usually under `.minecraft/bin/<uuid>`). 
The binary will be symlinked into the directory, or copied if not possible (mostly on
Windows). For shared objects files (.so) that contains version numbers in the filename,
these are discarded in the bin directory, for example 
`/lib/libglfw.so.3 -> .minecraft/bin/<uuid>/libglfw.so`.

These arguments can be used together to fix various issues (e.g. wrong libc being linked
by the LWJGL-provided natives).

> [!NOTE]
> Note that these arguments are compatible with, and executed after the `--lwjgl` 
> argument. You must however ensure that excluded lib and included binaries are 
> compatible.

#### Miscellaneous
With `--dry`, the start command does not start the game, but simply installs it.

With `--demo` you can enable the [demo mode](https://minecraft.wiki/w/Demo_mode) 
of the game.  

With `--resolution <width>x<height>` you can change the resolution of the game window.

The two arguments `--disable-mp` (mp: multiplayer) and `--disable-chat` can respectively
disable the multiplayer button and the in-game chat *(since 1.16)*.

### Search for versions
The `portablemc search [-k <kind>] [version]` command is used to search for versions. 
By default, this command will search for official Mojang versions available to download, 
you can instead search for many kinds of versions using the `-k` (`--kind`) arguments:
- `local`, show all installed versions.
- `forge`, show all recommended and latest Forge loader versions *(only 1.5.2 and 
  onward can be started)*.
- `fabric`, show all available Fabric loader versions.
- `legacyfabric`, show all available LegacyFabric loader versions.
- `quilt`, show all available Quilt loader versions.

The search string is optional, if not specified no filter is applied on the table shown.

### Authentication sessions
Two subcommands allow you to store or logout of sessions: `portablemc login|logout <email_or_username>`.
These subcommands don't prevent you from using the `-l` (`--login`) argument when starting
the game, these are just here to manage the session storage.

**By default**, this will authenticate you using the Microsoft authentication services,
you can change that using `--auth-service` argument, for example with `yggdrasil` if
you need to log into an old Mojang account (being phased out by Mojang).

**Your password is not saved!** Only tokens are saved *(the official launcher also does 
that)* in the file `portablemc_auth.json` in the working directory.

### Shell completion
The launcher can generate shell completions scripts for Bash and Zsh shells through the
`portablemc show completion {bash,zsh}` command. If you need precise explanation on how
to install the completions, read this command's help message. **This command needs to be
re-run for every new version of the launcher**, you're not affected if you directly eval
the result.

*Note that Zsh completion scripts can be used both as an auto-load script and as
evaluated one.*

## Offline support
This launcher can be used without internet access under certain conditions. Launching
versions is possible if all required resources are locally installed, it is also possible
to search for versions *(only Mojang, not Forge/Fabric/Quilt)* if the version manifest 
is locally cached, this can be forced by just running the search or start commands with
internet access, you can also copy the relevant files from an online computer to your
offline one. 
*Authentication commands and arguments are however not supported while offline.*

An example use case has been documented in issue [#178](https://github.com/mindstorm38/portablemc/issues/178#issuecomment-1752102655).

## Certifi support
The launcher supports [certifi](https://pypi.org/project/certifi/) when installed. 
This package provides *Mozilla’s carefully curated collection of Root Certificates for 
validating the trustworthiness of SSL certificates while verifying the identity of TLS 
hosts.* 

This can be useful if you encounter certificates errors while logging into your account
or downloading other things. Problems can arise because Python depends by default on your
system to provide these root certificates, so if your system is not up to date, it may be
necessary to install `certifi`.

## Contribute

### Setup environment
Conda (or Miniconda) is recommended for easy development together with Poetry.
If you want to try you can use the following commands:
```console
# You can use any version of Python here from 3.7 to test 
# compatibility of the launcher.
conda create -n pmc python=3.11 pip
# This line is optional if you don't have any user site-packages
# in your host installation, if not it allows to isolate pip. 
# This is useful to avoid conflicts with packages installed 
# outside of the environment.
conda env config vars set PYTHONNOUSERSITE=1 -n pmc
```

Once you have a conda environment, you can install the development version locally in it:
```console
# Assume we are in the project's directory.
# First, we need to activate the environment.
conda activate pmc
# If poetry isn't installed, or outdated 
# (minimum version tested is 1.5.0).
pip install poetry --upgrade
# Then you can install the portablemc package locally.
poetry install
# Now, you can test the development version of the launcher.
portablemc show about
```

You can call this development version from everywhere using:
```console
conda run -n pmc portablemc
```

### Contributors
This launcher would not be as functional without the contributors, and in particular the 
following for their bug reports, suggestions and pull requests to make the launcher 
better: 
[GoodDay360](https://github.com/GoodDay360), 
[Ristovski](https://github.com/Ristovski),
[JamiKettunen](https://github.com/JamiKettunen)
[MisileLaboratory](https://github.com/MisileLab) and
[GooseDeveloper](https://github.com/GooseDeveloper).

There must be a lot of hidden issues, if you want to contribute you just have to install 
and test the launcher, and report every issue you encounter, do not hesitate!

### Sponsors
I'm currently working on my open-source projects on my free time. So sponsorship is an
extra income that allows me to spend more time on the project! This can also help me
on other open-source projects. You can sponsor this project by donating either on
[GitHub Sponsors](https://github.com/sponsors/mindstorm38) or 
[Ko-fi](https://ko-fi.com/theorozier). I've always been passionate about open-source
programming and the relative success of PortableMC have been a first surprise to me, 
but the fact that people are now considering to support me financially is even more
rewarding! **Huge thanks to [Erwan Or](https://github.com/erwanor) and 
[user10072023github](https://github.com/user10072023github) for their donations!**
