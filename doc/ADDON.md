# PortableMC Addon API (W.I.P.)
The following document provides essential information for addon makers, this includes in particular the project 
workspace layout and the CLI interface. For a better understanding, you should read the [API Documentation](API.md) 
first. 

- [Environment](#environment)
- [Workspace](#workspace)
  - [Metadata file](#metadata-file)
  - [Entry point](#entry-point)
- [CLI reference](#cli-reference)
  - [Mixin](#mixin)

## Environment
No special development environment is needed to develop PMC addon's, however you can use an IDE, if you want to 
avoid highlighted errors or simply have a better autocompletion, you can install the `portablemc` package via PIP. 
**Note that this does not restrict the addon to be used on portable (single-script) installations, so don't 
hesitate to use this if this can be helpful.**

## Workspace
To create an addon, you need in the first place to make a directory named by your addon's identifier. This 
directory is also called the addon's "workspace", it will contain all Python files, metadata and resources. 
This directory can be distributed as-is to be placed in supported add-ons paths (`portablemc addon dirs`).

You should always test your addon while developing, to do this you can place its directory directly into
a parent directory that has its path in "addon dirs" (`portablemc addon dirs`), you can also add paths to 
the variable `PMC_ADDONS_PATH`.

Two files are required for a minimal addon:
- `addon.json`, the JSON metadata file with display name, description, more information in its own section
  [Metadata file](#metadata-file).
- `__init__.py`, the addon's entry point, like in a real python's package it will be called in the first
  place and load sub-packages if needed.

> In general, checking how official addons are working is a good way to understand the addon API. 

### Metadata file
The metadata file `addon.json` is a JSON formatted files, this file is required for an addon to be detected,
however all fields are optional, including name and version which are undefined in such case.

The following snippet shows an example with all fields:
```json
// addon.json
{
  "name": "Display name",
  "version": "1.0.0",
  "authors": ["Author One", "Author Two", "..."],
  "description": "Long description about my example addon.",
  "requires": { // This is no longer used but you can add it anyway as an information.
    "prompt_toolkit": ">=3.0.16, <3.1.0"
  }
}
```

### Entry point
The entry point file `__init__.py` is a simple Python file, you can also make modules and sub-packages next
to this file and import them using relative imports. For example if you have a file named `utils.py` next to
your entry point file, you can import it using `from . import utils` or `from .utils import myfunction`.

The launcher also allows you to import directly from PMC modules, using the same imports used for a PIP
installation. For example using `from portablemc.cli import print_table` or 
`from portablemc import StartOptions, cli as pmc`, so you can use `pmc.print_message()`.

Before going into details about what content you can write and where, you must know that PortableMC loads
addons before starting, it allows you to alter the command line argument parser and add/modify commands.
This loading happens in two times:
- Module loading, this step find all addons and load the `__init__.py` file, in this step, you should not
  interact nor alter external packages (like PMC or other addons).
- Addon loading, the launcher tries to find a `load` function, if it does, it is being called with a single
  parameter which is the CLI module itself. **This parameter should be ignored in modern version, it was used
  as retro-compatibility when directly importing PMC was not guaranteed.** In this step you can interact with
  external packages, for example to mixin external functions.

In general, **DO NOT** make expansive computation in these two steps, PMC is a CLI tool, and should compute
only when specific commands are called.

## CLI reference
The following chapters describes what functions are exposed by the CLI and for what you can use them in you 
addon.

### Mixin
Mixin is the core tool you can use to hook your addons into other addons or PMC. This tool allows you to replace
an existing function by a custom one, however this custom function take as first parameter the old function, this
allows it to extend or just ignore the old function.

This tool is implemented and exposed as a decorator called `mixin(name: str = None, into: str = None)`. The first
parameter `name` can be used if your custom function has not the same name has the function you want to mixin, by
default this parameter is set to the name of your function. Using the `into` argument, you can change the target
module or object where the original function should be replaced.

> Remember that you should not interact with external packages and addons until `load` function was called.

```python
from portablemc.cli import mixin

def load(_pmc):
  @mixin()
  def format_number(old, n: int) -> str:
      return old(n)
```

Like all decorators in python, you can use it like a regular function. The following example shows an example
equivalent to the previous one, but using delayed `mixin` call with custom name.


```python
from portablemc.cli import mixin

def my_format_number(old, n: int) -> str:
      return old(n)

def load(_pmc):
  mixin(name="format_number")(my_format_number)
```

## Argument parser
The launcher use the [argparse](https://docs.python.org/dev/library/argparse.html) API from the standard 
Python library. PMC use and expose a lot of function to register sub-commands and arguments, like
`register_arguments`, `register_subcommands` or `register_start_arguments` (check `cli.py` code to
have an exhaustive list), this allows you to customize every step of the process or add sub-commands and
arguments.

Additionally, you can mixin the function `get_command_handlers`, which must return a sub-command tree 
associating to each one an executor. Default executors (`cmd_search`, `cmd_start` and more) can also 
be mixed in to change behaviours of the commands.

