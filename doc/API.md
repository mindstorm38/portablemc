# PortableMC API

This document describes the PortableMC API usable via importing the `portablemc` module. This file will
not enter in details of classes or functions signatures, but will describe the concept of them and for
what they are used. Many classes and method provides docstring that describe signatures.

- [Core concepts](#core-concepts)
  - [Context](#context)
  - [VersionManifest](#versionmanifest)
  - [Version](#version)
  - [StartOptions](#startoptions)
  - [Start](#start)
- [Authentication](#authentication)
  - [AuthSession](#authsession)
  - [YggdrasilAuthSession](#yggdrasilauthsession)
  - [MicrosoftAuthSession](#microsoftauthsession)
  - [AuthDatabase](#authdatabase)
- [Download](#download)
  - [DownloadEntry](#downloadentry)
  - [DownloadList](#downloadlist)

## Core concepts

### Context
A context in PMC is used to keep track of core directories used to install and run Minecraft. It also
provides methods to list installed game versions or test if a specific version is existing. In current
version, a context stores paths to the following directories:
- `work_dir`, where the game actually run, where it stores saves, screenshots and 
  user-specific thing.
- `versions_dir`, versions' directories are stored with metadata and JAR file.
- `assets_dir`, game assets' directory, with indexes, object, skins...
- `libraries_dir`, where game libraries are stored.
- `jvm_dir`, where PMC downloads official versions of the Java Virtual Machine (JVM), it's specific
  to this launcher and does not exist in the official launcher.
- `bin_dir`, where temporary binaries (.dll, .so) are copied while game is running.

After constructing a context, all these attributes can be changed as you want.

### VersionManifest
A version manifest is an object that describes all online-available versions and how to download them. It also
gives the identifier of the latest snapshot and release. By default, this object will get its internal data
from `https://launchermeta.mojang.com/mc/game/version_manifest.json`, you can change this by extending the
class.

### Version
A version object in PMC is used to keep track of the installation of a version, this object is constructed
by giving a [Context](#context) object and the identifier (ID) of the version. The ID is the name of the 
directory where the version is installed, or if not installed, its identifier within its
[VersionManifest](#versionmanifest).

This class allows you to install a version step-by-step with the following methods:
- `prepare_meta()`, prepare all versions metadata required to install this version (with inheritance) and 
  computes the full flattened version metadata (without inheritance).
- `prepare_jar()`, prepare the version's JAR file if it's missing.
- `prepare_assets()`, prepare missing index and its assets (in context's `assets_dir`).
- `prepare_libraries()`, prepare missing JAR libraries (in context's `libraries_dir`).
- `prepare_jvm()`, prepare missing JVM files (in context's `jvm_dir`).
- `download(...)`, because all previous methods only "prepare" the launcher, actually downloading file is done via this
  method, it also improves global installation speed.

A simpler method `install` do all these steps for you (including download), it provides a keyword argument `jvm`
if you want to download the JVM.

After download is complete, you can use the `start` method to directly start the game, optionally you can pass
[StartOptions](#startoptions), check the [Start](#start) if you want to customize game starting.

> Note that installing JVM is not required to launch the game.

### StartOptions
A named tuple with options for starting the game, used when preparing process arguments. The following options
are available:
- `auth_session`, an optional [AuthSession](#authsession), if defined, it overrides `uuid` and `username`.
- `uuid`, an optional offline UUID (without dashes).
- `username`, an optional offline username.
- `demo`, start Minecraft in demo mode (disabled by default).
- `resolution`, optional resolution for the game's window.
- `disable_multiplayer`, defaults to `False` (available since 1.16).
- `disable_chat`, defaults to `False` (available since 1.16).
- `server_address`, an optional server address to connect after Minecraft has started (available since 1.6).
- `server_port`, an optional server port, only used by the game if `server_address``is defined (available since 1.6).
- `jvm_exec`, an optional JVM executable, by default it use the `jvm_exec` from the [Version](#version) object.
- `old_fix`, enable JVM arguments that partially fix the game in alpha, beta, and release between 1.0 and 1.5
  (included).
- `features`, a dictionary where you can define additional features, features are used to optionally change arguments
  version metadata files.

### Start
A start object is can be used to prepare arguments before starting the game, it is constructed with a 
[Version](#version) object, after that you can use the `prepare(opts)` method with [StartOptions](#startoptions).
After that you can use the `start()` method.

Start objects can be further customized to change the process runner or the binary directory path.

## Authentication

### AuthSession
This is a base class for different types of authentication protocols. Currently, it's implemented for
[Yggdrasil (Mojang)](#yggdrasilauthsession) and [Microsoft](#microsoftauthsession) protocols. This class 
is designed to be serialized and deserialized into a [AuthDatabase](#authdatabase), and provide some methods
for retro-compatibility of the database. Actual objects can be used in [StartOptions](#startoptions) to allows
client to have their skin and connect to online-mode servers.

### YggdrasilAuthSession
Implementation of the Yggdrasil (Mojang one) authentication protocol. Use `authenticate(client_id, email, password)`
to get a session object.

### MicrosoftAuthSession
Implementation of the Microsoft authentication protocol. This authentication protocol is a bit more complicated and
requires to log-in from a web browser. First, you need an "app id" from https://portal.azure.com/, also check
https://docs.microsoft.com/en-us/azure/active-directory/develop/howto-create-service-principal-portal. Remember to
make a "public" application (without secret key), and to authorize the redirect URI that you want to use.

Once your application is ready, use the `get_authentication_url(app_id, redirect_uri, email, nonce)`, note that
nonce can be used to check if the returned token ID is valid. Once you have a token, you can use 
`get_logout_url(app_id, redirect_uri)` to clear the browser cache, **the token will not be invalidated**.
You should also use `check_token_id(token_id, email, nonce)` to check if the user has logged-in using the
email and nonce given in `get_authentication_url`.

Finally, you can use `authenticate(client_id, app_id, code, redirect_uri)` to get a session object.

### AuthDatabase
An object linked to a database file, with explicit methods `load()`, `save()` and methods to manage its content:
- `get(email, sess_type)`, get a session by its email, and the session type (directly give the class).
- `put(email, sess)`, put a session in the database.
- `remove(email, sess_type)`, same as `get` but to remove a session from the database, returning it if existing.
- `get_client_id()`, can also be used to get a unique client ID (unique for the database) that you can use as a
  `client_id` for `authenticate` methods.

## Download
A utility API that provides efficient download for sets of files.

### DownloadEntry
An object that defines a single file to download, you should define when possible the expected size and/or SHA-1 hash
of the file. You can also define a display name for CLI or interfaces.

### DownloadList
A dynamic/growable list of [DownloadEntry](#downloadentry), when adding a download entry, public attributes `count`
and `size` and updated. You can also add callbacks functions that will be called if a download is successful. To
start the download, use `download_files(...)`.
