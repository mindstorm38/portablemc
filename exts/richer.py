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


def load(portablemc):

    from rich.progress import Progress, TaskID, BarColumn, TimeRemainingColumn, \
        TransferSpeedColumn, DownloadColumn, TextColumn
    from rich.console import Console
    from rich.table import Table

    console = Console(highlight=False)
    table: Optional[Table] = None
    progress: Optional[Progress] = None
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

        # "download.start": None,
        "download.progress": None,
        # "download.of_total": None,
        # "download.speed": None

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

    @replace(portablemc, "cmd_listext")
    def new_cmd_listext(old_cmd_listext, args: Namespace):
        res = old_cmd_listext(args)
        print_table()
        return res

    @replace(portablemc, "download_file")
    def new_download_file(old_download_file,
                          url: str, size: int, sha1: str, dst: str, *args,
                          total_size: int = 0, progress_callback=None, end_callback=None, **kwargs) -> Optional[int]:

        nonlocal progress, progress_task, total_progress_task

        progress = Progress(
            TextColumn("[progress.description]{task.description}", justify="right"),
            BarColumn(bar_width=None),
            "•",
            DownloadColumn(),
            "•",
            TransferSpeedColumn(),
            "•",
            TimeRemainingColumn(),
            console=console
        )

        # if total_size != 0:
        #     total_progress_task = progress.add_task("total", total=total_size)

        progress_task = progress.add_task(kwargs.get("name", url), total=size)
        progress.start()
        start_size = old_download_file(url, size, sha1, dst, *args,
                                       total_size=total_size,
                                       progress_callback=download_file_progress_callback,
                                       end_callback=None, **kwargs)
        progress.stop()

        progress = None
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

