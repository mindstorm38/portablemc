from argparse import Namespace
from typing import Optional


NAME = "Richer"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"


def replace(owner: object, name: str):
    old_val = getattr(owner, name, None)
    def decorator(func):
        def wrapper(*args, **kwargs):
            func(old_val, *args, **kwargs)
        setattr(owner, name, wrapper)
        return wrapper
    return decorator


def load(portablemc):

    try:
        import rich
        loaded = True
    except ModuleNotFoundError:
        loaded = False

    if not loaded:
        raise ValueError("Can't load 'richer' extension since 'rich' must be installed, use `pip install rich` or check https://github.com/willmcgugan/rich.")

    from rich.console import Console
    from rich.table import Table

    console = Console(highlight=False)
    table: Optional[Table] = None

    special_prints = {

        "cmd.search.pending": lambda args: set_table(Table(
            "Type", "Identifier", "Date",
            caption="Results for '{}'".format(args[0]),
        )),
        "cmd.search.result": lambda args: table.add_row(args[0], args[1], args[2]),
        "cmd.search.not_found": None,

        "cmd.listext.title": lambda args: set_table(Table(
            "Name", "Version", "Authors",
            caption="Extensions list ({})".format(args[0])
        )),
        "cmd.listext.result": lambda args: table.add_row(args[0], args[1], args[2])

    }

    @replace(portablemc, "print")
    def new_print(_, message_key: str, *args, traceback: bool = False):

        if message_key in special_prints:
            special_print = special_prints[message_key]
            if callable(special_print):
                special_print(args)
            return

        console.print(portablemc.get_message(message_key, *args))

        if traceback:
            console.print_exception()

    @replace(portablemc, "cmd_search")
    def new_cmd_search(old_cmd_search, args: Namespace):
        nonlocal table
        res = old_cmd_search(args)
        console.print()
        console.print(table)
        console.print()
        table = None
        return res

    @replace(portablemc, "cmd_listext")
    def new_cmd_listext(old_cmd_listext, args: Namespace):
        nonlocal table
        res = old_cmd_listext(args)
        console.print()
        console.print(table)
        console.print()
        table = None
        return res

    def set_table(new_table: Table):
        nonlocal table
        table = new_table
