from argparse import Namespace
from typing import Optional


NAME = "Richer"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "rich"


def replace(owner: object, name: str):
    old_val = getattr(owner, name, None)
    def decorator(func):
        def wrapper(*args, **kwargs):
            return func(old_val, *args, **kwargs)
        setattr(owner, name, wrapper)
        return wrapper
    return decorator


def safe_delete(owner: dict, name: str):
    if name in owner:
        del owner[name]


def load(portablemc):

    from rich.progress import Progress, TaskID, BarColumn, TimeRemainingColumn, \
        TransferSpeedColumn, DownloadColumn
    from rich.console import Console, Theme
    from rich.table import Table

    theme = Theme({
        "progress.download": "",
        "progress.data.speed": "",
        "progress.remaining": ""
    })

    console = Console(highlight=False, theme=theme)
    table: Optional[Table] = None

    progress = Progress(
        "=> [progress.description]{task.description} •",
        BarColumn(),
        "•",
        DownloadColumn(),
        "•",
        TransferSpeedColumn(),
        "•",
        TimeRemainingColumn(),
        console=console
    )

    progress_task: Optional[TaskID] = None
    total_progress_task: Optional[TaskID] = None

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
        "cmd.listext.result": lambda args: table.add_row(args[0], args[1], args[2]),

        "download.progress": None,

    }

    @replace(portablemc, "print")
    def new_print(_, message_key: str, *args, traceback: bool = False, end: str = "\n"):

        if message_key in special_prints:
            special_print = special_prints[message_key]
            if callable(special_print):
                special_print(args)
            return

        console.print(portablemc.get_message(message_key, *args), end=end)

        if traceback:
            console.print_exception()

    @replace(portablemc, "cmd_search")
    def new_cmd_search(old_cmd_search, args: Namespace):
        res = old_cmd_search(args)
        print_table()
        return res

    @replace(portablemc, "cmd_start")
    def new_cmd_start(old_cmd_start, args: Namespace):
        res = old_cmd_start(args)
        return res

    @replace(portablemc, "cmd_listext")
    def new_cmd_listext(old_cmd_listext, args: Namespace):
        res = old_cmd_listext(args)
        print_table()
        return res

    @replace(portablemc, "run_game")
    def new_run_game(old_run_game, proc_args, proc_cwd):

        from rich.layout import Layout
        from rich.live import Live

        layout = Layout()
        layout_game_log = Layout(name="game_logs")

        layout.split(layout_game_log)

        with Live(layout, screen=True, refresh_per_second=4, console=console):
            pass

        old_run_game(proc_args, proc_cwd)

    @replace(portablemc, "download_file")
    def new_download_file(old_download_file,
                          entry,
                          *args,
                          total_size: int = 0,
                          **kwargs) -> Optional[int]:

        nonlocal progress_task, total_progress_task

        safe_delete(kwargs, "progress_callback")
        safe_delete(kwargs, "end_callback")

        progress_task = progress.add_task(entry.name, total=entry.size)

        with progress:

            start_size = old_download_file(entry,
                                           *args,
                                           total_size=total_size,
                                           progress_callback=download_file_progress_callback,
                                           end_callback=None,
                                           **kwargs)

        progress_task = None
        total_progress_task = None
        return start_size

    def download_file_progress_callback(dl_size: int, _size: int, dl_total_size: int, _total_size: int):
        progress.update(progress_task, completed=dl_size)
        if total_progress_task is not None:
            progress.update(total_progress_task, completed=dl_total_size)

    def set_table(new_table: Table):
        nonlocal table
        table = new_table

    def print_table():
        nonlocal table
        console.print()
        console.print(table)
        console.print()
        table = None

