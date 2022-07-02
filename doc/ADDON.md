# PortableMC Addon API (DEPRECATED SINCE 3.0.0, FIXME)
The following document provides essential information for addon makers, this includes in particular the project 
workspace layout and the CLI interface. For a better understanding, you should read the [API Documentation](API.md) 
first. 

- [Environment](#environment)
- [Workspace](#workspace)
  - [Metadata file](#metadata-file)
  - [Entry point](#entry-point)
- [CLI reference](#cli-reference)
  - [Mixin](#mixin)
  - [Argument parser](#argument-parser)
  - [Printing](#printing)
    - [Messages](#messages)
    - [Simple printing](#simple-printing)
    - [Table printing](#table-printing)
    - [Task printing](#task-printing)

## Environment
No special development environment is needed to develop PMC addon's. If you want to use an IDE in order to 
have a better autocompletion for example, you can install the `portablemc` package via PIP. 
**Note that this does not restrict the addon to be used on portable (single-script) installations, so don't 
hesitate to use this if this can be helpful.**

## Workspace
To create an addon, you need in the first place to make a directory named by your addon's identifier. This 
directory is also called the addon's "workspace", and contains all Python files, metadata and resources.
Because you should always test your addon while developing, place it into a directory that is define in the
"addons path" (`portablemc addon dirs`), you can also add custom paths to the variable `PMC_ADDONS_PATH`. 

Two files are required for a minimal addon:
- `addon.json`, the JSON metadata file with display name, description for example.
- `__init__.py`, the addon's entry point, like in a real python's package it will be called in the first
  place and load sub-packages if needed.

> In general, checking how existing addons are working is a good way to understand the addon API. 

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
your entry point file, you can import it with `from . import utils` or `from .utils import myfunction`.

The launcher also allows you to import directly from PMC modules, using the same imports you would use with 
a PIP installation. For example using `from portablemc.cli import print_table` or 
`from portablemc import StartOptions, cli as pmc`, so you can use `pmc.print_message()`. *Note that the CLI
module `portablemc.cli` imports all the core module, so `from portablemc.cli import StartOptions` work, 
**but don't use this as it might be removed in future versions**, so please import from the right module*.

Before going into details about what content you can write and where, you must know that PortableMC loads
addons before starting, it allows them to alter the command line argument parser and add/modify commands.
This loading happens in two times:
- Module loading, this step find all addons and load the `__init__.py` file, in this step, you should not
  interact nor alter external packages (like PMC or other addons).
- Addon loading, the launcher tries to find a `load` function, if it does, it is being called with a single
  parameter which is the CLI module itself. **This parameter should be ignored in modern version, it was used
  as retro-compatibility when directly importing PMC was not guaranteed.** In this step you can interact with
  external packages, for example to mixin external functions.

In general, **DON'T** make expansive computation in these two steps, PMC is a CLI tool, and should compute
only when commands are called.

## CLI reference
The following chapters describes what functions are exposed by the CLI and for what you can use them in you 
addon. Following description are based on an import looking like `from portablemc import cli as pmc`.

### Mixin
Mixin is the core tool exposed to hook into other addons or PMC. It allows you to replace existing functions 
by custom ones, however your custom functions take as first parameter the old function, which allows them to 
extend or just ignore the old function.

This tool is implemented and exposed as a decorator called `mixin(name=None, into=None)`. The 
first parameter `name` can be used if your custom function has not the same name has the function you want to 
mixin, by default this parameter is set to the name of your function. Using the `into` argument, you can 
change the target module or object where the original function should be replaced.

> Remember that you should not interact with external packages and addons until `load` function was called.

```python
def load(_pmc):
  @pmc.mixin()
  def format_number(old, n: int) -> str:
      return old(n)
```

Like all decorators in python, you can use it like a regular function. The following example shows an example
equivalent to the previous one, but using delayed `mixin` call with custom name.


```python
def my_format_number(old, n: int) -> str:
      return old(n)

def load(_pmc):
  pmc.mixin(name="format_number")(my_format_number)
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

## Printing
Because PMC is a CLI tool, you might want to print messages to the terminal. The launcher exposes multiple
functions and a global message dictionary.

### Messages
First, the messages' dictionary is accessible via `pmc.messages`, it stores every translatable messages 
used by the launcher and addons, each message is indexed by its key. To define your own messages you can 
use `pmc.messages.extend({"my_key": "my msg", ...})`. This dictionary can also be used to make translation 
addons.

Two methods allows you to get messages from it: `pmc.get_message_raw(key, kwargs=None)` and 
`pmc.get_message(key, **kwargs)`. These two methods allows you to get messages from their key but also 
allows you to format the message if it exists, if it doesn't the key is returned. The difference between 
the two method is how they are called, the first "raw" one takes an optional dictionary and the "non-raw" 
one take keyword arguments. For example:

```python
pmc.messages["my_addon.test"] = "Test message with: {param}"
pmc.get_message_raw("my_addon.test", {"param": "value"})
pmc.get_message("my_addon.test", param="value")
# both outputs: Test message with: value
```

> Messages can be debugged using `portablemc show lang`.

### Simple printing
Use the function `pmc.print_message(key, kwargs=None, *, end="\n", trace=False, critical=False)` to print
messages from the messages' dictionary. This function is based on the `get_message_raw` function described
above and prints the result to the console. Additionally, the `end` argument can be used to change print a 
specific string as end-of-line, the `trace` argument can be used to print the stack trace after the message
and the `critical` argument set the foreground color to red.

### Table printing
If you want to print pretty tables, you can use `pmc.print_table(lines, *, header=-1)`. Lines must be a list
of tuple, all the same length. Each element in a tuple is a cell on the line. The optional `header` argument
can be used to separate all lines smaller or equals by their index to the argument.

### Task printing
The launcher also provides task printing function
`pmc.print_task(status, msg_key, msg_args, *, done=False, keep_previous=False)`. You can use it to print a 
line with a status in brackets and update this line until the task is done. This is inspired by systemd 
started logging.

Task printing example:
```python
pmc.messages["my_addon.task.pending"] = "Task is pending..."
pmc.messages["my_addon.task.done"] = "Task done!"
pmc.messages["my_addon.after"] = "Message after."
pmc.print_task("..", "my_addon.task.pending")
time.sleep(5)
pmc.print_task("OK", "my_addon.task.done", done=True)
pmc.print_task(None, "my_addon.after", done=True)

# python example.py
# [  ..  ] Task is pending...

# and 5 seconds later, the line is updated:

# python example.py
# [  OK  ] Task done!
#          Message after.
#
```
