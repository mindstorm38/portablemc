

NAME = "Richer"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "rich", "prompt_toolkit"


def ext_build():

    from typing import cast, Optional, TextIO
    from prompt_toolkit.layout.controls import FormattedTextControl, BufferControl
    from prompt_toolkit.layout.containers import Window, HSplit, VSplit
    from prompt_toolkit.key_binding.key_processor import KeyPressEvent
    from prompt_toolkit.application import Application
    from prompt_toolkit.key_binding import KeyBindings
    from prompt_toolkit.document import Document
    from prompt_toolkit.buffer import Buffer
    from prompt_toolkit.layout import Layout
    from queue import Queue, Full, Empty
    from subprocess import Popen, PIPE
    from threading import Thread
    import asyncio

    class RicherExtension:

        def __init__(self, pmc):
            self.pmc = pmc

        def load(self):
            self.pmc.add_message("cmd.start.richer.title", "Minecraft {} • {} • {}")
            self.pmc.mixin("run_game", self.run_game)

        def run_game(self, _old, proc_args: list, proc_cwd: str, options: dict):

            title_text = self.pmc.get_message("cmd.start.richer.title",
                                              options.get("version", "unknown_version"),
                                              options.get("username", "anonymous"),
                                              options.get("uuid", "uuid"))

            buffer_window = LimitedBufferWindow(100)

            container = HSplit(children=[
                VSplit(children=[
                    Window(width=2),
                    Window(content=FormattedTextControl(text=title_text)),
                ], height=1, style="bg:#005fff fg:black"),
                VSplit(children=[
                    Window(width=1),
                    buffer_window,
                    Window(width=1)
                ])
            ])

            keys = KeyBindings()

            application = Application(
                layout=Layout(container),
                key_bindings=keys,
                full_screen=True
            )

            process = Popen(proc_args, cwd=proc_cwd, stdout=PIPE, stderr=PIPE, bufsize=1, universal_newlines=True)

            @keys.add("c-c")
            def _exit(event: KeyPressEvent):
                event.app.exit()

            async def _run_process():
                stdout_reader = ThreadedProcessReader(cast(TextIO, process.stdout))
                stderr_reader = ThreadedProcessReader(cast(TextIO, process.stderr))
                while True:
                    code = process.poll()
                    if code is None:
                        buffer_window.append(stdout_reader.poll(), stderr_reader.poll())
                        await asyncio.sleep(0.1)
                    else:
                        break

            async def _run():
                _done, _pending = await asyncio.wait((_run_process(), application.run_async()),
                                                     return_when=asyncio.FIRST_COMPLETED)
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                if application.is_running:
                    application.exit()

            asyncio.get_event_loop().run_until_complete(_run())

    class LimitedBufferWindow:

        def __init__(self, limit: int):
            self.buffer = Buffer(read_only=True)
            self.string_buffer = RollingStringBuffer(limit)
            self.window = Window(content=BufferControl(buffer=self.buffer))

        def append(self, *texts: str):
            modified = False
            for text in texts:
                if self.string_buffer.append(text):
                    modified = True
            if modified:
                cursor_pos = None
                new_text = self.string_buffer.get()
                if self.buffer.cursor_position < len(self.buffer.text):
                    cursor_pos = self.buffer.cursor_position
                self.buffer.set_document(Document(text=new_text, cursor_position=cursor_pos), bypass_readonly=True)

        def __pt_container__(self):
            return self.window

    class RollingStringBuffer:

        def __init__(self, limit: int):
            self._strings = []
            self._limit = limit

        def append(self, txt: Optional[str]) -> bool:
            if txt is not None and len(txt):
                self._strings.append(txt)
                while len(self._strings) > self._limit:
                    self._strings.pop(0)
                return True
            else:
                return False

        def get(self) -> str:
            return "".join(self._strings)

    class ThreadedProcessReader:

        def __init__(self, in_stream: TextIO):
            self._input = in_stream
            self._queue = Queue(100)
            self._thread = Thread(target=self._entry, daemon=True)
            self._thread.start()

        def _entry(self):
            for line in iter(self._input.readline, b''):
                try:
                    self._queue.put_nowait(line)
                except Full:
                    pass
            self._input.close()

        def poll(self) -> Optional[str]:
            try:
                return self._queue.get_nowait()
            except Empty:
                return None

    return RicherExtension





"""def replace(owner: object, name: str):
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

    from typing import cast, Optional, TextIO
    from queue import Queue, Empty, Full
    from argparse import Namespace
    from threading import Thread

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

    portablemc.add_message("cmd.start.richer.title", "Minecraft {} • {} • {}")


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
    def new_run_game(_old_run_game, proc_args: list, proc_cwd: str, options: dict):

        from prompt_toolkit.layout.controls import FormattedTextControl, BufferControl
        from prompt_toolkit.layout.containers import Window, HSplit, VSplit
        from prompt_toolkit.key_binding.key_processor import KeyPressEvent
        from prompt_toolkit.application import Application
        from prompt_toolkit.key_binding import KeyBindings
        from prompt_toolkit.document import Document
        from prompt_toolkit.buffer import Buffer
        from prompt_toolkit.layout import Layout
        from subprocess import Popen, PIPE
        import asyncio

        terminal_buffer = Buffer(read_only=True)
        terminal_string_buffer = RollingStringBuffer(100)

        title_text = portablemc.get_message("cmd.start.richer.title",
                                            options.get("version", "unknown_version"),
                                            options.get("username", "anonymous"),
                                            options.get("uuid", "uuid"))

        container = HSplit(children=[
            VSplit(children=[
                Window(width=2),
                Window(content=FormattedTextControl(text=title_text)),
            ], height=1, style="bg:#005fff fg:black"),
            Window(height=1),
            VSplit(children=[
                Window(width=1),
                Window(content=BufferControl(buffer=terminal_buffer)),
                Window(width=1)
            ])
        ])

        keys = KeyBindings()

        application = Application(
            layout=Layout(container),
            key_bindings=keys,
            full_screen=True
        )

        process = Popen(proc_args, cwd=proc_cwd, stdout=PIPE, stderr=PIPE, bufsize=1, universal_newlines=True)

        @keys.add("c-c")
        def _exit(event: KeyPressEvent):
            event.app.exit()

        def _update_buffers():
            terminal_buffer.set_document(Document(terminal_string_buffer.get()), bypass_readonly=True)

        async def _run_process():
            stdout_reader = ThreadedProcessReader(cast(TextIO, process.stdout))
            stderr_reader = ThreadedProcessReader(cast(TextIO, process.stderr))
            while True:
                code = process.poll()
                if code is None:
                    terminal_string_buffer.append(stdout_reader.poll())
                    terminal_string_buffer.append(stderr_reader.poll())
                    _update_buffers()
                    await asyncio.sleep(0.1)
                else:
                    break

        async def _run():
            _done, _pending = await asyncio.wait((_run_process(), application.run_async()), return_when=asyncio.FIRST_COMPLETED)
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            if application.is_running:
                application.exit()

        asyncio.get_event_loop().run_until_complete(_run())


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


    class RollingStringBuffer:

        def __init__(self, limit: int):
            self._strings = []
            self._limit = limit

        def append(self, txt: Optional[str]):
            if txt is not None and len(txt):
                self._strings.append(txt)
                while len(self._strings) > self._limit:
                    self._strings.pop(0)

        def get(self) -> str:
            return "".join(self._strings)


    class ThreadedProcessReader:

        def __init__(self, in_stream: TextIO):
            self._input = in_stream
            self._queue = Queue(100)
            self._thread = Thread(target=self._entry, daemon=True)
            self._thread.start()

        def _entry(self):
            for line in iter(self._input.readline, b''):
                try:
                    self._queue.put_nowait(line)
                except Full:
                    pass
            self._input.close()

        def poll(self) -> Optional[str]:
            try:
                return self._queue.get_nowait()
            except Empty:
                return None
"""