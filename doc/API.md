# PortableMC API
This page documents the public API of the launcher. This launcher
library provides high flexibility for launching Minecraft in many
environments.

Documented version: `4.0.0`.

## Table of contents
- [File structure](#file-structure)
- [Standard version](#standard-version)
  - [Hello world!](#hello-world)
  - [Version name](#version-name)
  - [Version context](#version-context)
  - [Watch events](#watch-events)
  - [Version fixes](#version-fixes)
  - [Other options](#other-options)
  - [Environment](#environment)
- [Fabric/Quilt version](#fabricquilt-version)
- [Forge version](#forge-version)
- [Versioning and stability](#versioning-and-stability)

## File structure
Sources of the launcher's API are stored in the `src/portablemc` 
directory. Files are described in the following tree list:
- `__init__.py`, root of the library, contains launcher name, version,
  authors, copyright and URL;
- `download.py`, optimized parallel download classes and tasks;
- `http.py`, collection of simple functions to make simple HTTP API
  requests with better response/error classes;
- `util.py`, global misc utilities without particular classification;
- `auth.py`, base classes for authentication;
- `standard.py`, base classes required to launch standard versions
  provided by Mojang;
- `fabric.py`: fabric/quilt mod loaders support;
- `forge.py`: forge mod loader support.

## Standard version
The standard version is the most basic type of version, it allows you to start any Mojang
version, install it and run it with simple functions. The game's installation finds 
missing resources and download them in parallel, allowing fast installation and 
low-latency startup for already installed versions.

### Hello world!

Following piece of code is the simplest method to install and run the game with default
parameters (latest release):
```python
from portablemc.standard import Version
Version().install().run()
```

To understand the different steps involved, let's rewrite the previous code with 
explanatory comments:
```python
# Create a version installer for latest release
version = Version()
# Ensure that version is installed, returning the 
# game's environment needed to run it.
env = version.install()
# Start the game and wait for it to close.
env.run()
```

### Version name

This example uses default parameters, but you can specify a lot of parameters to run the
game, starting with the version to run:
```python
version = Version("1.20.1")    # A particular release
version = Version("23w18a")    # A particular snapshot
version = Version("release")   # Latest release
version = Version("snapshot")  # Latest snapshot
```

### Version context

Another thing that can be specified is the context of the game, the context describe 
which directories to use for various resources, such as jvm, libraries, assets or 
versions. The two main kind of directories are "main directory" and "work directory",
the main directory stores resources as described previously, and work directory stores
user's personal files such as worlds, resource packs and options. By default, both 
are set to the standard `.minecraft`.

The context can be specified at version's instantiation:
```python
from portablemc.standard import Context, Version
from pathlib import Path

# Here's an example where main directory is '/opt/minecraft'
# and work dir is '~/.minecraft'.
context = Context(Path("/opt/minecraft"), Path.home() / ".minecraft")
# Use this context for our version.
version = Version(context=context)
```

### Watch events

You can watch events produced by the installation by providing a watcher to the `install`
method, a watcher must be a subclass of the `Watcher` class:
```python
class MyWatcher(Watcher):
    def handle(self, event) -> None:
        print("install event", event)

version.install(watcher=MyWatcher())
```

Events are differentiated by their class, these classes has the suffix `Event` and are
available in the same `standard` module, for example `VersionLoadingEvent`. 
Documentation of each event can help you understand how they are triggered.

### Version fixes

The version class allows for various *fixes* to be applied on various game's parts, in
order to fix well-known issues with old versions. Fixes are string constant defined in
the `Version` class prefixed with `FIX_`, the following fixes are supported:

- `FIX_LEGACY_PROXY`, add HTTP proxies to old versions' arguments in order to adapt old
  request to newer Mojang's APIs, such as skins. Possible thanks to 
  [betacraft](https://betacraft.uk/).
- `FIX_LEGACY_MERGE_SORT`, add a JVM flag for fixing crashes on old beta/alpha versions.
- `FIX_LEGACY_RESOLUTION`, old versions doesn't support default window resolution by
  default in their metadata, in such case this fix adds resolution arguments if relevant.
- `FIX_LEGACY_QUICK_PLAY`, old versions doesn't support quick play arguments in their
  metadata, this fix adds the proper arguments to automatically connect. *This only works
  with multiplayer server quick play, for now.*
- `FIX_1_16_AUTH_LIB`, versions 1.16.4 and 1.16.5 of Minecraft use Mojang's authlib
  version `2.1.28` which cause multiplayer to be disabled in offline mode, this can be 
  fixed by replacing it with the next version `2.2.30`.
- `FIX_LWJGL`, unlike previous fixes, this one takes a particular LWJGL version, it will
  force the game to use the given LWJGL version. *This can be used to support ARM devices
  which are not supported by default by Mojang.* Supported versions are `3.2.3`, `3.3.0`,
  `3.3.1` and `3.3.2`, specifying other version will raise an exception.

All fixes except LWJGL are enabled by default *(good defaults!)*, you can enable or 
disable fixes like this:
```python
# Disable fix of authlib 2.1.28
version.fixes[Version.FIX_AUTH_LIB_2_1_28] = False
# Enable force use of LWJGL 3.3.2
version.fixes[Version.FIX_LWJGL] = "3.3.2"
```

You can gather the applied fixes in the environment returned by the `install` method:
```python
env = version.install()
print(env.fixes[Version.FIX_AUTH_LIB_2_1_28])  # True
print(env.fixes[Version.FIX_LWJGL])  # 3.3.2
```

### Other options

Various options are available on version's instances before starting the installation:
- `demo`, enable demo mode (may not be supported on all versions).
- `auth_session`, the authentication session to use to be authenticated in-game:
  - `set_auth_offline(username, uuid)`, this function is a shortcut to setup an offline
    authentication mode for standard the game.
- `resolution`, optional initial resolution for the game's window.
- `disable_multiplayer`, force disable multiplayer.
- `disable_chat`, force disable chat.
- `quick_play`, optional quick play mode for starting the game:
  - `set_quick_play_singleplayer(level_name)`, shortcut function to setup a singleplayer
    quick play to a given level name;
  - `set_quick_play_multiplayer(host, port = 25565)`, shortcut function to setup a server
    multiplayer quick play given a host and optional port;
  - `set_quick_play_realms(realm)`, shortcut function to setup a realm multiplayer quick
    play to a given realm identifier.
- `jvm_path`, optional JVM executable path to force this to be used for running the game.

### Environment

The environment returned by `Version.install()` method can be modified before running the
game, it's an instance of `Environment` and contains the following attributes:
- `jvm_args`, the list of arguments passed to the JVM, the first argument is the JVM 
  executable path. You can use it to add custom JVM arguments for memory, like `-Xmx`.
- `game_args`, the list of arguments passed to Minecraft.
- `main_class`, the main class started by the JVM, usually something like 
  `net.minecraft.client.main.Main`.
- `args_replacements`, a mapping of variables names to values, these will be used to
  replace variables of the form `${my_var}` in both `jvm_args` and `game_args`.
- `native_libs`, a list of native libraries or shared objects that are copied (or 
  symlinked if relevant) in the game's temporary bin directory.
- `fixes`, this one as no effect for the game's runtime, it's just a summary of fixes 
  applied during the game's installation, that affected the environment.

## Fabric/Quilt version

The fabric mod loader is supported by default, using the `FabricVersion` class. Both 
Fabric and Quilt are supported by this class because those mod loaders have the same
install process and API endpoints:
```python
from portablemc.fabric import FabricVersion

fabric_version = FabricVersion.with_fabric()
quilt_version = FabricVersion.with_quilt()
```

These versions can be installed and run like standard ones, and much like standard
versions you can specify which version to run *(the same applies for both fabric and 
quilt)*:
```python
# Fabric for Minecraft 1.20.1, with latest fabric loader
version = FabricVersion.with_fabric("1.20.1")
# Fabric for Minecraft 1.20.1, with fabric loader v0.14.21
version = FabricVersion.with_fabric("1.20.1", "0.14.21")
# Context can be specified like standard version
version = FabricVersion.with_fabric(context=my_context)
```

The real name of a fabric or quilt version *(as stored inside the versions directory)*
will be by default `fabric|quilt-<vanilla_version>-<loader_version>`. You can change
the prefix using the `prefix` argument of the constructors:
```python
# Using format: my-fabric-<vanilla_version>-<loader_version>
version = FabricVersion.with_fabric(prefix="my-fabric")
```

## Forge version

The forge mod loader is supported by default, using the `ForgeVersion`:
```python
from portablemc.forge import ForgeVersion

# Forge for latest version
version = ForgeVersion()
# Forge for Minecraft 1.20.1, recommended (both equivalent)
version = ForgeVersion("1.20.1")
version = ForgeVersion("1.20.1-recommended")
# Forge for Minecraft 1.20.1, latest
version = ForgeVersion("1.20.1-latest")
# Forge for Minecraft 1.20.1, explicit version
version = ForgeVersion("1.20.1-47.1.0")
```

Same as fabric, this version type is a subclass of standard version and therefore can be
installed and run in the same way. Note however that because of the huge complexity of
supporting a wide variety of forge installer versions (all forge versions with an 
installer are supported), this version's installation is more complex and might be longer 
due to "processors" (executables parts of the forge's installer that build the version).

The real name of a forge version *(as stored inside the versions directory)*
will be by default `forge-<forge_version>` You can change the prefix using the `prefix` 
argument of the constructors:
```python
# Using format: my-forge-<forge_version>
version = ForgeVersion(prefix="my-forge")
```

## Versioning and stability
This launcher uses [semantic versioning](https://semver.org/), the version is defined by 
three numbers: major, minor and patch with the format `<major>.<minor>.<patch>`. Release
candidates are suffixed with `rc<num>`. Semantic versioning defines three cases when
incrementing the version's numbers:

1. Major number is incremented when incompatible *API* changes are made;
2. Minor number is incremented when new features are added to the *API* in a backward 
   compatible manner;
3. Patch number is incremented when backward compatible changes are made but no feature
   is added.

The *API* is defined by all symbols publicly available to users of the PortableMC library,
it is composed of all symbols (classes, variables, functions), modules and arguments that
are not prefixed by one or more underscore "_", *this is a common convention in Python*.
Note that only keyword arguments can be prefixed with an underscore, which means that the
arguments is made for internal use or is experimental.

Because of the dynamic design of Python, it's unstable *(may break at any time)* to
externally inject symbols to the portablemc's modules and classes, because the API might 
define such symbol in the future. For example, a subclass of an API's class may break at 
any time if defining methods or attributes, because those might be defined on superclasses 
in any future version. It is also unstable to override a method, because arguments may be 
added in any future versions and break your override *(you can use `*args` and `**kwargs` 
to make it stable)*.

Many symbols are private, this allows greater flexibility for developing the launcher, 
but if you think that some members should be made public, you can open an issue.

Here are examples of stable and unstable usage of the library:
```python
# Import API symbol: stable
from portablemc.standard import Version, Watcher
# Import a private symbol: unstable
from portablemc.download import _DownloadEntry

version = Version()
# Access API attribute: stable
version.demo = True
# Access private attribute: unstable
version._assets_index_version = "..."
# Inject a new attribute: unstable
version.foo = 42

# Subclass of an API's class is stable, following important details...
class MyVersion(Version):

    # It's *stable* to override using variadic arguments, because 
    # this adapts to future function signatures.
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # It's *unstable* to define your own argument, because the
        # same attribute may be defined in the future on Version class
        self.bar = "..."
        # You can reduce the risk by using uncommon prefixes.
        self._my_version_bar = "..."
        # Or by using name mangling built in Python (at your own stability risk!).
        self.__bar = "..."
    
    # It's *unstable* to override with parent signature, because
    # this doesn't adapt for future function signatures.
    def set_auth_offline(self, username: Optional[str], uuid: Optional[str]) -> None:
        super().set_auth_offline(username, uuid)
    
    # Same as above, but *stable* because future signatures are handled.
    def set_auth_offline(self, username: Optional[str], uuid: Optional[str], *args, **kwargs) -> None:
        super().set_auth_offline(username, uuid, *args, **kwargs)
    
    # It's *unstable* to define your own methods because the same
    # name might get defined in the future class' signature.
    def baz(self):
        pass

# Example of stable subclass of the standard Watcher class
class MyWatcher(Watcher):
    def handle(self, event: Any, *args, **kwargs) -> None:
        print(f"here's an event: {event}")
```
