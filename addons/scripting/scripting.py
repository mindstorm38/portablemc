from prompt_toolkit.key_binding.bindings.focus import focus_next, focus_previous
from prompt_toolkit.layout.containers import Window, HSplit, VSplit, Container
from prompt_toolkit.key_binding.key_processor import KeyPressEvent
from prompt_toolkit.layout.controls import FormattedTextControl
from prompt_toolkit.layout.processors import BeforeInput
from prompt_toolkit.key_binding import KeyBindings
from prompt_toolkit.application import Application
from prompt_toolkit.filters import Condition
from prompt_toolkit.widgets import TextArea
from prompt_toolkit.buffer import Buffer

from prompt_toolkit.lexers import PygmentsLexer
from pygments.lexers.python import PythonLexer

from typing import List, Optional, Dict, Callable, Any
from argparse import ArgumentParser, Namespace
from threading import Thread
from os import path

from ..richer.richer import RollingLinesWindow
from .server import ScriptingServer


JAR_FILE_PATH = path.join(path.dirname(__file__), "java/out/portablemc_scripting.jar")


class ScriptingAddon:

    def __init__(self, pmc):

        self.pmc = pmc
        self.richer = None

        self.server: 'Optional[ScriptingServer]' = None
        self.active = False

    def load(self):

        self.richer = self.pmc.get_addon("richer").instance
        self.richer.double_exit = True

        self.pmc.add_message("args.start.scripting", "Enable the scripting extension injection at startup.")
        self.pmc.add_message("start.scripting.start_server", "Scripting server started on port {}.")
        self.pmc.add_message("start.scripting.title", "Live Scripting • port: {}")

        self.pmc.mixin("register_start_arguments", self.register_start_arguments)
        self.pmc.mixin("start_game", self.start_game)
        self.pmc.mixin("build_application", self.build_application, self.richer)

    def register_start_arguments(self, old, parser: ArgumentParser):
        parser.add_argument("--scripting", help=self.pmc.get_message("args.start.scripting"), default=False, action="store_true")
        old(parser)

    def start_game(self, old, *, raw_args: Namespace, **kwargs) -> None:

        if raw_args.scripting:

            self.server = ScriptingServer()
            self.active = True

            def libraries_modifier(classpath_libs: List[str], _native_libs: List[str]):
                classpath_libs.append(JAR_FILE_PATH)

            def args_modifier(args: List[str], main_class_index: int):
                self.server.start()
                self.pmc.print("start.scripting.start_server", self.server.get_port())
                old_main_class = args[main_class_index]
                args[main_class_index] = "portablemc.scripting.ScriptingClient"
                args.insert(main_class_index, "-Dportablemc.scripting.main={}".format(old_main_class))
                args.insert(main_class_index, "-Dportablemc.scripting.port={}".format(self.server.get_port()))

            kwargs["libraries_modifier"] = libraries_modifier
            kwargs["args_modifier"] = args_modifier

        old(raw_args=raw_args, **kwargs)

        if raw_args.scripting:
            self.server.stop()

    def build_application(self, old, container: Container, keys: KeyBindings) -> Application:

        interpreter = None

        if self.active:

            title_text = self.pmc.get_message("start.scripting.title", self.server.get_port())
            interpreter = Interpreter(self.server)

            container = VSplit([
                container,
                Window(char=' ', width=1, style="class:header"),
                HSplit([
                    VSplit([
                        Window(width=2),
                        Window(FormattedTextControl(text=title_text)),
                    ], height=1, style="class:header"),
                    VSplit([
                        Window(width=1),
                        interpreter,
                        Window(width=1)
                    ])
                ])
            ])

            keys.add("tab", filter=~Condition(interpreter.require_key_tab))(focus_next)
            keys.add("s-tab", filter=~Condition(interpreter.require_key_tab))(focus_previous)

        app = old(container, keys)

        if self.active:
            app.layout.focus(interpreter.input)

        return app


class Interpreter:

    INDENTATION_SPACES = 4

    def __init__(self, server: 'ScriptingServer'):

        # Customize the default builtins, and use DynamicDict
        # in order to avoid reflection at startup time.
        builtins = DynamicDict(globals()["__builtins__"])
        builtins["print"] = self._custom_print
        del builtins["help"]
        del builtins["input"]
        del builtins["breakpoint"]

        # Add customized builtins, types and minecraft dynamic
        # value and also all builtins class wrappers.
        from . import std, mc
        builtins["types"] = server.types
        builtins.add_dyn("mc", lambda: mc.Minecraft.get_instance(server))
        for mod_name, cls_name, cls in self.iter_modules_classes(std, mc):
            builtins[cls_name] = cls

        self.locals = {}
        self.globals = {
            "__builtins__": builtins
        }

        self.code_indent = 0
        self.code = []

        self.lexer = PygmentsLexer(PythonLexer)
        self.window = RollingLinesWindow(100, lexer=self.lexer, wrap_lines=True, dont_extend_height=True)
        self.input = TextArea(
            height=1,
            multiline=False,
            wrap_lines=False,
            accept_handler=self._input_accept,
            lexer=self.lexer
        )

        self.prompt_processor = BeforeInput(">>> ", "")
        self.input.control.input_processors.clear()
        self.input.control.input_processors.append(self.prompt_processor)

        keys = KeyBindings()
        keys.add("tab", filter=Condition(self.require_key_tab))(self._handle_tab)

        self.split = HSplit([
            Window(),
            self.window,
            self.input
        ], key_bindings=keys)

    # Printing

    def print_line(self, line: str):
        self.window.append(*line.splitlines())

    def _custom_print(self, *args, sep: str = " ", **_kwargs):
        self.print_line(sep.join(str(arg) for arg in args))

    def _out_traceback(self):
        import traceback
        import sys
        err_type, err, tb = sys.exc_info()
        for line in traceback.extract_tb(tb).format()[2:]:
            self.print_line("{}".format(line))
        self.print_line("{}: {}".format(err_type.__name__, err))

    # Interpreting

    def _interpret(self, text: str):

        self.print_line("{}{}".format(self.prompt_processor.text, text))

        text_spaces = self.count_spaces(text)

        if text_spaces % self.INDENTATION_SPACES != 0:
            self.print_line("Unexpected identation.")
            self._insert_identation(self.code_indent)
            return

        text_indent = text_spaces // self.INDENTATION_SPACES

        if text_indent > self.code_indent:
            self.print_line("Unexpected identation.")
            self._insert_identation(self.code_indent)
            return

        self.code_indent = text_indent
        if len(text):
            self.code.append(text)
            if text.rstrip().endswith(":"):
                self.code_indent += 1
                self.prompt_processor.text = "... "

        if self.code_indent == 0 and len(self.code):
            self.prompt_processor.text = ">>> "
            eval_text = "\n".join(self.code)
            self.code.clear()
            try:
                ret = eval(eval_text, self.globals, self.locals)
                self.print_line("{}".format(ret))
            except SyntaxError:
                try:
                    exec(eval_text, self.globals, self.locals)
                except (BaseException,):
                    self._out_traceback()
            except (BaseException,):
                self._out_traceback()

        self._insert_identation(self.code_indent)

    def _input_accept(self, buffer: Buffer) -> bool:
        Thread(target=lambda: self._interpret(buffer.text), daemon=True).start()
        return False

    def _handle_tab(self, _e: KeyPressEvent):
        if self.code_indent != 0:
            self._insert_identation(1)

    def _insert_identation(self, count: int):
        self.input.document = self.input.document.insert_before(" " * count * self.INDENTATION_SPACES)

    # Other

    def require_key_tab(self) -> bool:
        return self.code_indent != 0

    def __pt_container__(self):
        return self.split

    # Utils

    @staticmethod
    def count_spaces(line: str) -> int:
        i = 0
        for c in line:
            if c == " ":
                i += 1
            else:
                break
        return i

    @staticmethod
    def iter_modules_classes(*modules):
        for module in modules:
            mod_name = module.__name__
            for key in dir(module):
                raw = getattr(module, key)
                if isinstance(raw, type):
                    yield mod_name, key, raw


class DynamicDict(dict):

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self._dyns: 'Dict[str, Callable[[], Any]]' = {}

    def add_dyn(self, key, dyn: 'Callable[[], Any]'):
        self._dyns[key] = dyn
        self[key] = None

    def __getitem__(self, item):
        val = super().__getitem__(item)
        if val is None:
            dyn = self._dyns.get(item)
            if dyn is not None:
                val = self[item] = dyn()
        return val
