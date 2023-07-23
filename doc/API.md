# PortableMC API
This page documents the public API of the launcher. This launcher
library provides high flexibility for launching Minecraft in many
environments.

Documented version: `4.0.0`.

## Table of contents
- [Versioning](#versioning)
- [File structure](#file-structure)
- [Tasks](#tasks)

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

## Tasks
The main concepts of this launcher are tasks, sequences and states.
Tasks are objects that can be added to a sequence object, the sequence
object can then be *executed*, this will run all tasks sequentially,
each task then modifies the shared state. Additionally, a watcher
mechanism allows tasks to dispatch events while executing.

These classes can be found under `portablemc.tasks` module:
- `State`, a particular dictionary used to keep the state of a 
  sequence, it maps the value to its own type, this allows great 
  static type analysis;
- `Task`, base class for tasks, provides to functions to redefine:
  - `setup`, called when the task is added to a sequence, the task
    can use this to add default states;
  - `execute`, called when the task is executed;
- `Watcher`, base class for event watchers;
- `Sequence`, sequence class to use to prepend/append tasks, manually
  add states and then execute all of them.

## Vanilla tasks
This module provides standard tasks for running officially provided
versions. It also provides primitive classes for defining the 
running context of the game and querying Mojang's version manifest. 

These classes can be found under `portablemc.vanilla`, there is many 
tasks and we do not document them here for now.

This module also expose function for easily constructing a sequence: 
`add_vanilla_tasks(sequence, *, run = False)`, this function extends 
the given sequence with all required tasks in the right order, you can 
optionally decide to run the game or not. 

**However**, this function doesn't add base states required to run 
the game:
- `Context`, the context of the game are its directories;
- `VersionRepositories`, the version repositories to use to resolve missing 
  versions;
- `MetadataRoot`, the root version to start resolving metadata for 
  when executing the sequence.

To fix this, you can use an easier function that does all the work for
you, `make_vanilla_sequence`, as seen in the following example:
```py
from portablemc.vanilla import make_vanilla_sequence

fn main():
    # Make a sequence for installing and running 1.20.1
    seq = make_vanilla_sequence("1.20.1", run=True)
    # Executing this sequence will install and run the game.
    seq.execute()
```
