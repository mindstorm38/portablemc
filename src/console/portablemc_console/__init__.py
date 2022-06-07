from prompt_toolkit.layout.controls import FormattedTextControl, BufferControl
from prompt_toolkit.layout.containers import Window, HSplit, VSplit, Container
from prompt_toolkit.key_binding.key_processor import KeyPressEvent
from prompt_toolkit.formatted_text import StyleAndTextTuples
from prompt_toolkit.application import Application
from prompt_toolkit.key_binding import KeyBindings
from prompt_toolkit.document import Document
from prompt_toolkit.filters import Filter
from prompt_toolkit.layout import Layout
from prompt_toolkit.buffer import Buffer
from prompt_toolkit.lexers import Lexer
from prompt_toolkit.styles import Style

from typing import cast, Optional, TextIO, Callable, List
from asyncio import Queue, QueueFull, QueueEmpty
from argparse import ArgumentParser, Namespace
from subprocess import Popen, PIPE
from threading import Thread
import asyncio
import shutil

from portablemc import Version, Start
from portablemc import cli as pmc


def load():

    # Private mixins

    @pmc.mixin()
    def register_start_arguments(old, parser: ArgumentParser):
        _ = pmc.get_message
        parser.add_argument("--no-console", help=_("args.start.no_console"), action="store_true")
        parser.add_argument("--single-exit", help=_("args.start.single_exit"), action="store_true")
        old(parser)

    @pmc.mixin()
    def new_start(old, ctx: pmc.CliContext, version: Version) -> Start:
        start = old(ctx, version)
        old_runner = start.runner
        def runner_wrapper(args: List[str], cwd: str):
            bin_dir = start.args_replacements["natives_directory"]
            runner(old_runner, bin_dir, args, cwd, ctx.ns, start)
        start.runner = runner_wrapper
        return start

    # Messages

    pmc.messages.update({
        "args.start.no_console": "Disable the process' console from the 'console' addon.",
        "args.start.single_exit": "For richer terminal, when Minecraft process is terminated, do "
                                  "not ask for Ctrl+C to effectively exit the terminal.",
        "start.console.title": "Minecraft {version} • {username} • {uuid}",
        "start.console.command_line": "Command line: {line}",
        "start.console.confirm_close": "Minecraft process has terminated, Ctrl+C again to close terminal."
    })


def build_application(container: Container, keys: KeyBindings) -> Application:
    return Application(
        layout=Layout(container),
        key_bindings=keys,
        full_screen=True,
        style=Style([
            ("header", "bg:#005fff fg:black")
        ])
    )


def runner(old, bin_dir: str, args: List[str], cwd: str, ns: Namespace, start: Start) -> None:

    if ns.no_console:
        old(args, cwd)
        return

    _ = pmc.get_message

    title_text = _("start.console.title", version=start.version.id, username=start.get_username(), uuid=start.get_uuid())

    buffer_window = RollingLinesWindow(400, lexer=ColoredLogLexer(), last_line_return=True)
    buffer_window.append(_("start.console.command_line", line=" ".join(args)), "")

    container = HSplit([
        VSplit([
            Window(width=2),
            Window(FormattedTextControl(text=title_text)),
        ], height=1, style="class:header"),
        VSplit([
            Window(width=1),
            buffer_window,
            Window(width=1)
        ])
    ])

    keys = KeyBindings()
    double_exit = not ns.single_exit

    @keys.add("c-c")
    def _exit(event: KeyPressEvent):
        nonlocal process
        if not double_exit or process is None:
            event.app.exit()
        else:
            process.kill()

    @keys.add("c-w")
    def _switch_wrapping(_event: KeyPressEvent):
        buffer_window.switch_wrap_lines()

    application = build_application(container, keys)
    process = Popen(args, cwd=cwd, stdout=PIPE, stderr=PIPE, bufsize=1, universal_newlines=True)

    async def _run_process():
        nonlocal process
        stdout_reader = ThreadedProcessReader(cast(TextIO, process.stdout))
        stderr_reader = ThreadedProcessReader(cast(TextIO, process.stderr))
        while True:
            code = process.poll()
            if code is None:
                done, pending = await asyncio.wait((
                    stdout_reader.poll(),
                    stderr_reader.poll()
                ), return_when=asyncio.FIRST_COMPLETED)
                for done_task in done:
                    line = done_task.result()
                    if line is not None:
                        buffer_window.append(line)
                for pending_task in pending:
                    pending_task.cancel()
            else:
                stdout_reader.wait_until_closed()
                stderr_reader.wait_until_closed()
                buffer_window.append(*stdout_reader.poll_all(), *stderr_reader.poll_all())
                break
        process = None
        shutil.rmtree(bin_dir, ignore_errors=True)
        if double_exit:
            buffer_window.append("", _("start.console.confirm_close"))

    async def _run():
        _done, _pending = await asyncio.wait((
            _run_process(),
            application.run_async()
        ), return_when=asyncio.ALL_COMPLETED if double_exit else asyncio.FIRST_COMPLETED)
        if process is not None:
            process.kill()
            process.wait(timeout=5)
        if application.is_running:
            application.exit()

    asyncio.get_event_loop().run_until_complete(_run())


class RollingLinesWindow:

    def __init__(self, limit: int, *,
                 lexer: Optional[Lexer] = None,
                 dont_extend_height: bool = False,
                 last_line_return: bool = False):

        self.last_line_return = last_line_return
        self.buffer = Buffer(read_only=True)
        self.string_buffer = RollingLinesBuffer(limit)
        self.wrap_lines = False

        class WrapLinesFilter(Filter):

            def __init__(self, win: RollingLinesWindow):
                self.win = win

            def __call__(self) -> bool:
                return self.win.wrap_lines

        self.window = Window(
            content=BufferControl(buffer=self.buffer, lexer=lexer, focusable=True),
            wrap_lines=WrapLinesFilter(self),
            dont_extend_height=dont_extend_height
        )

    def get_wrap_lines(self) -> bool:
        return self.wrap_lines

    def switch_wrap_lines(self):
        self.wrap_lines = not self.wrap_lines

    def append(self, *lines: str):
        if self.string_buffer.append(*lines):
            cursor_pos = None
            new_text = self.string_buffer.get()
            if self.last_line_return:
                new_text += "\n"
            if self.buffer.cursor_position < len(self.buffer.text):
                cursor_pos = self.buffer.cursor_position
            self.buffer.set_document(Document(text=new_text, cursor_position=cursor_pos), bypass_readonly=True)

    def __pt_container__(self):
        return self.window


class RollingLinesBuffer:

    def __init__(self, limit: int):
        self._strings = []
        self._limit = limit

    def append(self, *lines: str) -> bool:
        if not len(lines):
            return False
        for line in lines:
            if not len(line):
                self._strings.append("")
            else:
                self._strings.extend(line.splitlines())
        while len(self._strings) > self._limit:
            self._strings.pop(0)
        return True

    def get(self) -> str:
        return "\n".join(self._strings)


class ThreadedProcessReader:

    def __init__(self, in_stream: TextIO):
        self._input = in_stream
        self._queue = Queue(100)
        self._thread = Thread(target=self._entry, daemon=True)
        self._thread.start()
        self._closed = False

    def _entry(self):
        try:
            for line in iter(self._input.readline, ""):
                try:
                    self._queue.put_nowait(line)
                except QueueFull:
                    pass
            self._input.close()
        except ValueError:
            pass
        try:
            self._queue.put_nowait("")
        except QueueFull:
            pass

    def wait_until_closed(self):
        self._input.close()
        self._thread.join(5000)

    async def poll(self) -> Optional[str]:
        if self._closed:
            return None
        val = await self._queue.get()
        if not len(val):
            self._closed = True
        return None if self._closed else val

    def poll_all(self):
        try:
            val = self._queue.get_nowait()
            while val is not None:
                yield val
                val = self._queue.get_nowait()
        except QueueEmpty:
            pass


class ColoredLogLexer(Lexer):

    def lex_document(self, document: Document) -> Callable[[int], StyleAndTextTuples]:
        lines = document.lines

        def get_line(lineno: int) -> StyleAndTextTuples:

            try:

                line = lines[lineno]

                tmp_line = line
                tmp_lineno = lineno
                got_exception = False

                def has_exception() -> bool:
                    nonlocal got_exception
                    got_exception = "Exception" in tmp_line
                    return got_exception

                while tmp_line[0] == "\t" or has_exception():
                    tmp_lineno -= 1
                    tmp_line = lines[tmp_lineno]
                    if tmp_lineno < 0:
                        return []
                    if got_exception:
                        break

                if "WARN" in tmp_line:
                    style = "#ffaf00"
                elif "ERROR" in tmp_line or got_exception:
                    style = "#ff005f"
                elif "FATAL" in tmp_line:
                    style = "#bf001d"
                else:
                    style = ""

                return [(style, line.replace("\t", "    "))]

            except IndexError:
                return []

        return get_line

