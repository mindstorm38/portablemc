# PortableMC API
This page documents the public API of the launcher. This launcher
library provides high flexibility for launching Minecraft in many
environments.

Documented version: `4.0.0`.

## Table of contents
- [Versioning](#versioning)
- [File structure](#file-structure)

## Versioning
This launcher uses [semantic versioning](https://semver.org/lang/fr/),
therefore you should expect no breaking changes in the public API when
bumping the minor or patch version numbers. **Please note** however
that the `cli` module should not be considered part of the public API,
even if functions appear public, **the CLI is an implementation 
detail**, only its command line interface follows semantic versioning
guarantees.

## File structure
Sources of the launcher's API are stored in the `src/portablemc` 
directory. Files are described in the following tree list:
- `__init__.py`, root of the library, contains launcher name, version,
  authors, copyright and URL;
- `task.py`, the tasks system base classes;
- `download.py`, optimized parallel download classes and tasks;
- `http.py`, collection of simple functions to make simple HTTP API
  requests with better response/error classes;
- `util.py`, global misc utilities without particular classification;
- `auth.py`, base classes for authentication;
- `vanilla.py`, base classes required to launch standard versions
  provided by Mojang;
- `lwjgl.py`: support for runtime fix of LWJGL version;
- `fabric.py`: fabric/quilt mod loaders support;
- `forge.py`: forge mod loader support.

