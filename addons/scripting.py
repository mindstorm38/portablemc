
NAME = "Scripting"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "addon:richer", "prompt_toolkit"


# Candidate client tick hooks for every version:
# - 1.14.4 - 1.16 'Queue<Runnable> Minecraft.progressTasks' (or 'Minecraft.tell(Runnable)')


def addon_build(pmc_in):

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

    from typing import List, Callable, Optional, Union, Tuple, Any
    from argparse import ArgumentParser, Namespace
    from abc import ABCMeta, abstractmethod
    from threading import Thread
    from enum import Enum
    from os import path
    import socket
    import struct
    import time


    TEMP_JAR_FILE_PATH = path.join(path.dirname(__file__), "scripting_dev/out/artifacts/portablemc_scripting_dev_jar/portablemc_scripting_dev.jar")


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
                    classpath_libs.append(TEMP_JAR_FILE_PATH)

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

        def build_application(self, old, container: Container, keys: KeyBindings) -> Application:

            interpreter = None

            if self.active:

                title_text = self.pmc.get_message("start.scripting.title", self.server.get_port())
                interpreter = Interpreter(self.server.get_context(), self.richer)

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

        def __init__(self, context: 'ScriptingContext', richer_addon_instance):

            builtins = dict(globals()["__builtins__"])
            builtins["print"] = self._custom_print
            del builtins["help"]
            del builtins["input"]
            del builtins["breakpoint"]

            self.locals = {}
            self.globals = {
                "ctx": context,
                "ty": context.types,
                "TextComponent": TextComponent,
                "__builtins__": builtins
            }

            self.code_indent = 0
            self.code = []

            self.lexer = PygmentsLexer(PythonLexer)
            self.window = richer_addon_instance.RollingLinesWindow(100, lexer=self.lexer, wrap_lines=True, dont_extend_height=True)
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

        @staticmethod
        def count_spaces(line: str) -> int:
            i = 0
            for c in line:
                if c == " ":
                    i += 1
                else:
                    break
            return i

        def __pt_container__(self):
            return self.split

    """def build_completer():
        import keyword
        return WordCompleter(list(keyword.kwlist), ignore_case=True)"""

    # TCP server and reflected objects definitions

    PACKET_GET_CLASS = 1
    PACKET_GET_FIELD = 2
    PACKET_GET_METHOD = 3
    PACKET_FIELD_GET = 10
    PACKET_FIELD_SET = 11
    PACKET_METHOD_INVOKE = 20
    PACKET_OBJECT_GET_CLASS = 30
    PACKET_RESULT = 100
    PACKET_RESULT_CLASS = 101

    class ScriptingServer:

        def __init__(self):

            self._context = ScriptingContext(self)
            self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self._port: Optional[int] = None

            self._client_socket: Optional[socket.socket] = None
            self._tx_buf = ByteBuffer(4096)
            self._rx_buf = ByteBuffer(4096)

            self._rx_recv_buf = bytearray(256)

            self._put_value_int_encoders = {
                "byte": (-2, ByteBuffer.put),
                "short": (-3, ByteBuffer.put_short),
                "int": (-4, ByteBuffer.put_int),
                "long": (-5, ByteBuffer.put_long),
                "float": (-6, ByteBuffer.put_float),
                "double": (-7, ByteBuffer.put_double),
                "char": (-8, ByteBuffer.put_char)
            }

        def get_context(self) -> 'ScriptingContext':
            return self._context

        def start(self):

            self._socket.bind(('127.0.0.1', 0))
            self._port = self._socket.getsockname()[1]

            thread = Thread(target=self._entry, name="PortableMC Scripting Server Thread", daemon=True)
            thread.start()

        def stop(self):
            self._socket.close()

        def get_port(self) -> int:
            return self._port

        def _entry(self):
            self._socket.listen(1)
            self._client_socket, _ = self._socket.accept()
            while True:
                time.sleep(1)

        # Packets

        def _prepare_packet(self):
            self._tx_buf.clear()
            self._tx_buf.ensure_len(3)

        def _send_packet(self, packet_type: int):
            length = self._tx_buf.pos
            self._tx_buf.put(packet_type, offset=0)
            self._tx_buf.put_short(length - 3, offset=1)
            self._client_socket.sendall(self._tx_buf.data[:length])

        def _wait_for_packet(self, expected_packet_type: int) -> 'ByteBuffer':

            next_packet_len = 0
            self._rx_buf.clear()

            while True:

                if next_packet_len == 0 and self._rx_buf.pos >= 3:
                    next_packet_len = self._rx_buf.get_short(offset=1, signed=False) + 3

                if next_packet_len != 0 and next_packet_len >= self._rx_buf.pos:
                    packet_type = self._rx_buf.get(offset=0)
                    self._rx_buf.limit = next_packet_len
                    self._rx_buf.pos = 3
                    if packet_type == expected_packet_type:
                        return self._rx_buf
                    else:
                        self._rx_buf.lshift(next_packet_len)
                        next_packet_len = 0
                        print("[SCRIPTING] Invalid received packet type, expected {}, got {}.".format(
                            expected_packet_type, packet_type))
                else:
                    remaining = self._rx_buf.remaining()
                    read_len = self._client_socket.recv_into(self._rx_recv_buf, min(len(self._rx_recv_buf), remaining))
                    self._rx_buf.put_bytes(self._rx_recv_buf, read_len)

        # Packets implementations

        def send_get_class_packet(self, class_name: str) -> 'Optional[ReflectClass]':
            self._prepare_packet()
            self._tx_buf.put_string(class_name)
            self._send_packet(PACKET_GET_CLASS)
            idx = self._wait_for_packet(PACKET_RESULT).get_int()
            return None if idx == -1 else ReflectClass(self._context, idx, class_name)

        def send_get_field_packet(self, owner: 'ReflectClass', field_name: str, field_type: 'ReflectClass') -> 'Optional[ReflectField]':
            self._prepare_packet()
            self._tx_buf.put_int(owner.internal_index)
            self._tx_buf.put_string(field_name)
            self._tx_buf.put_int(field_type.internal_index)
            self._send_packet(PACKET_GET_FIELD)
            idx = self._wait_for_packet(PACKET_RESULT).get_int()
            return None if idx == -1 else ReflectField(self._context, idx, owner, field_name, field_type)

        def send_get_method_packet(self, owner: 'ReflectClass', method_name: str, parameter_types: 'Tuple[ReflectClass, ...]') -> 'Optional[ReflectExecutable]':
            # Empty method name means we want a constructor
            self._prepare_packet()
            self._tx_buf.put_int(owner.internal_index)
            self._tx_buf.put_string(method_name)
            self._tx_buf.put(len(parameter_types))
            for ptype in parameter_types:
                self._tx_buf.put_int(ptype.internal_index)
            self._send_packet(PACKET_GET_METHOD)
            idx = self._wait_for_packet(PACKET_RESULT).get_int()
            if idx == -1:
                return None
            elif len(method_name):
                return ReflectMethod(self._context, idx, owner, method_name, parameter_types)
            else:
                return ReflectConstructor(self._context, idx, owner, parameter_types)

        def send_field_get_packet(self, field: 'ReflectField', owner: 'Optional[ReflectObject]') -> 'AnyReflectType':
            self._prepare_packet()
            self._tx_buf.put_int(field.internal_index)
            self._tx_buf.put_int(-1 if owner is None else owner.internal_index)
            self._send_packet(PACKET_FIELD_GET)
            return self._get_value(self._wait_for_packet(PACKET_RESULT))

        def send_field_set_packet(self, field: 'ReflectField', owner: 'Optional[ReflectObject]', val: 'AnyReflectType'):
            self._prepare_packet()
            self._tx_buf.put_int(field.internal_index)
            self._tx_buf.put_int(-1 if owner is None else owner.internal_index)
            self._put_value(self._tx_buf, val, field.get_type())
            self._send_packet(PACKET_FIELD_SET)
            self._wait_for_packet(PACKET_RESULT)

        def send_method_invoke_packet(self, method: 'ReflectExecutable', owner: 'Optional[ReflectObject]', parameters: 'Tuple[AnyReflectType, ...]') -> 'AnyReflectType':
            param_types = method.get_parameter_types()
            if len(param_types) != len(parameters):
                raise ValueError("Parameters count doesn't match, got {}, expected {}.".format(len(parameters), len(param_types)))
            self._prepare_packet()
            self._tx_buf.put_int(method.internal_index)
            self._tx_buf.put_int(-1 if owner is None else owner.internal_index)
            self._tx_buf.put(len(parameters))
            for idx, param in enumerate(parameters):
                self._put_value(self._tx_buf, param, param_types[idx])
            self._send_packet(PACKET_METHOD_INVOKE)
            return self._get_value(self._wait_for_packet(PACKET_RESULT))

        def send_object_get_class_packet(self, obj: 'ReflectObject') -> Optional[Tuple[str, int]]:
            self._prepare_packet()
            self._tx_buf.put_int(obj.internal_index)
            self._send_packet(PACKET_OBJECT_GET_CLASS)
            buf = self._wait_for_packet(PACKET_RESULT_CLASS)
            idx = buf.get_int()
            if idx == -1:
                return None
            else:
                return buf.get_string(), idx

        # Decode reflect value

        def _get_value(self, buf: 'ByteBuffer') -> 'AnyReflectType':
            idx = buf.get_int()
            if idx < 0:
                if idx == -2:
                    return buf.get()
                elif idx == -3:
                    return buf.get_short()
                elif idx == -4:
                    return buf.get_int()
                elif idx == -5:
                    return buf.get_long()
                elif idx == -6:
                    return buf.get_float()
                elif idx == -7:
                    return buf.get_double()
                elif idx == -8:
                    return buf.get_char()
                elif idx == -9:
                    return buf.get_string()
                elif idx == -10:
                    return False
                elif idx == -11:
                    return True
                else:
                    return None
            else:
                return ReflectObject(self._context, idx)

        def _put_value(self, buf: 'ByteBuffer', val: 'AnyReflectType', target_type: 'ReflectClass'):
            if val is None:
                if target_type.is_primitive():
                    raise ValueError("None value is illegal for primitive type {}.".format(target_type.get_name()))
                buf.put_int(-1)
            elif isinstance(val, bool):  # 'bool' must be placed before 'int' because 'bool' extends 'int'
                if target_type.get_name() != "boolean":
                    raise ValueError("Boolean {} given but expected {}.".format(val, target_type.get_name()))
                buf.put_int(-11 if val else -10)
            elif isinstance(val, int):
                data = self._put_value_int_encoders.get(target_type.get_name())
                if data is None:
                    raise ValueError(
                        "Integer value {} is not suitable for {} type.".format(val, target_type.get_name()))
                buf.put_int(data[0])
                (data[1])(buf, val)
            elif isinstance(val, str):
                if target_type.get_name() != "java.lang.String":
                    raise ValueError("String '{}' given but expected {}.".format(val, target_type.get_name()))
                buf.put_int(-9)
                buf.put_string(val)
            else:
                buf.put_int(val.internal_index)

    class ScriptingContext:

        def __init__(self, server: ScriptingServer):
            self._server = server
            self._types = TypesCache(self)
            self._minecraft: 'Optional[Minecraft]' = None

        def get_server(self) -> 'ScriptingServer':
            return self._server

        @property
        def types(self) -> 'TypesCache':
            return self._types

        @property
        def minecraft(self) -> 'Minecraft':
            if self._minecraft is None:
                self._minecraft = Minecraft.get_instance(self)
            return self._minecraft

        def __str__(self):
            return "<ScriptingContext>"

    class TypesCache:

        def __init__(self, ctx: ScriptingContext):
            self._ctx = ctx
            self._server = ctx.get_server()
            self._types = {}

        def _get(self, name) -> 'Optional[ReflectClass]':
            if hasattr(name, "CLASS_NAME"):
                name = str(getattr(name, "CLASS_NAME"))
            typ = self._types.get(name)
            if typ is None:
                typ = self._types[name] = self._server.send_get_class_packet(name)
                if typ is None:
                    raise ClassNotFoundError("Class '{}' not found.".format(name))
            return typ

        def raw_ensure(self, name: str, idx: int) -> Optional['ReflectClass']:
            typ = self._types.get(name)
            if typ is None:
                typ = self._types[name] = ReflectClass(self._ctx, idx, name)
            return typ

        def __getattr__(self, item) -> 'Optional[ReflectClass]':
            return self._get(item)

        def __getitem__(self, item) -> 'Optional[ReflectClass]':
            return self._get(item)

        def __str__(self):
            return "<TypesCache>"

    class ReflectObject:

        __slots__ = "_ctx", "_idx", "_class"

        def __init__(self, ctx: ScriptingContext, idx: int):
            self._ctx = ctx
            self._idx = idx
            self._class: 'Optional[ReflectClass]' = None

        @property
        def context(self) -> 'ScriptingContext':
            return self._ctx

        @property
        def internal_index(self) -> int:
            return self._idx

        def get_class(self) -> 'ReflectClass':
            if self._class is None:
                res = self._ctx.get_server().send_object_get_class_packet(self)
                if res is None:
                    raise ValueError("Unexpected null class was returned.")
                else:
                    self._class = self._ctx.types.raw_ensure(res[0], res[1])
            return self._class

        def __str__(self):
            return "<Object #{}>".format(self._idx)

    AnyReflectType = Union[ReflectObject, int, float, bool, str, None]

    class ReflectClass(ReflectObject):

        __slots__ = "_name",

        def __init__(self, ctx: ScriptingContext, idx: int, name: str):
            super().__init__(ctx, idx)
            self._name = name

        def get_name(self) -> str:
            return self._name

        def get_field(self, name: str, field_type: 'ReflectClass') -> 'ReflectField':
            field = self._ctx.get_server().send_get_field_packet(self, name, field_type)
            if field is None:
                raise FieldNotFoundError()
            return field

        def get_method(self, name: str, *parameter_types: 'ReflectClass') -> 'ReflectMethod':
            if not len(name):
                raise ValueError("Empty method name.")
            method = self._ctx.get_server().send_get_method_packet(self, name, parameter_types)
            if method is None:
                raise MethodNotFoundError()
            return method

        def get_constructor(self, *parameter_types: 'ReflectClass') -> 'ReflectConstructor':
            constructor = self._ctx.get_server().send_get_method_packet(self, "", parameter_types)
            if constructor is None:
                raise MethodNotFoundError()
            return constructor

        def is_primitive(self) -> bool:
            return self._name in ("byte", "short", "int", "long", "float", "double", "boolean", "char")

        def __str__(self):
            return "<Class {}#{}>".format(self._name, self._idx)

    class ReflectClassMember(ReflectObject):
        __slots__ = "_owner", "_name"
        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, name: str):
            super().__init__(ctx, idx)
            self._owner = owner
            self._name = name

    class ReflectField(ReflectClassMember):

        __slots__ = "_type"

        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, name: str, field_type: ReflectClass):
            super().__init__(ctx, idx, owner, name)
            self._type = field_type

        def get_type(self) -> ReflectClass:
            return self._type

        def get(self, owner: Optional[ReflectObject]) -> AnyReflectType:
            return self._ctx.get_server().send_field_get_packet(self, owner)

        def get_static(self) -> AnyReflectType:
            return self.get(None)

        def set(self, owner: Optional[ReflectObject], val: AnyReflectType):
            self._ctx.get_server().send_field_set_packet(self, owner, val)

        def set_static(self, val: AnyReflectType):
            self.set(None, val)

        def __str__(self):
            return "<Field {} {}.{}>".format(self._type.get_name(), self._owner.get_name(), self._name)

    class ReflectExecutable(ReflectClassMember):

        __slots__ = "_parameter_types"

        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, name: str, parameter_types: 'Tuple[ReflectClass, ...]'):
            super().__init__(ctx, idx, owner, name)
            self._parameter_types = parameter_types

        def get_parameter_types(self) -> 'Tuple[ReflectClass, ...]':
            return self._parameter_types

    class ReflectMethod(ReflectExecutable):

        __slots__ = ()

        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, name: str, parameter_types: 'Tuple[ReflectClass, ...]'):
            super().__init__(ctx, idx, owner, name, parameter_types)

        def invoke(self, owner: Optional[ReflectObject], *parameters: AnyReflectType) -> AnyReflectType:
            return self._ctx.get_server().send_method_invoke_packet(self, owner, parameters)

        def invoke_static(self, *parameters: AnyReflectType) -> AnyReflectType:
            return self.invoke(None, *parameters)

        def __call__(self, owner: Optional[ReflectObject], *parameters: AnyReflectType) -> AnyReflectType:
            return self.invoke(owner, *parameters)

        def __str__(self):
            return "<Method {}.{}({})>".format(
                self._owner.get_name(),
                self._name,
                ", ".format(*(typ.get_name for typ in self._parameter_types))
            )

    class ReflectConstructor(ReflectExecutable):

        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, parameter_types: 'Tuple[ReflectClass, ...]'):
            super().__init__(ctx, idx, owner, "<init>", parameter_types)

        def construct(self, *parameters: AnyReflectType) -> ReflectObject:
            return self._ctx.get_server().send_method_invoke_packet(self, None, parameters)

        def __str__(self):
            return "<Method {}({})>".format(
                self._owner.get_name(),
                ", ".format(*(typ.get_name for typ in self._parameter_types))
            )

    class ClassNotFoundError(Exception): ...
    class FieldNotFoundError(Exception): ...
    class MethodNotFoundError(Exception): ...

    # Cached class members

    class MemberCache(metaclass=ABCMeta):

        __slots__ = "_member", "_supplier"

        def __init__(self, supplier: Callable[[], tuple]):
            self._supplier = supplier
            self._member = None

        def get(self, ctx: ScriptingContext):
            if self._member is None or self._member.context != ctx:
                self._member = self._provide(ctx, *self._supplier())
            return self._member

        @abstractmethod
        def _provide(self, ctx: ScriptingContext, *args): ...

    class FieldCache(MemberCache):

        def get(self, ctx: ScriptingContext) -> ReflectField:
            return super().get(ctx)

        def _provide(self, ctx, *args) -> ReflectField:
            class_name, field_name, field_type_name = args
            cls = ctx.types[class_name]
            return cls.get_field(field_name, ctx.types[field_type_name])

    class MethodCache(MemberCache):

        def get(self, ctx: ScriptingContext) -> ReflectMethod:
            return super().get(ctx)

        def _provide(self, ctx, *args) -> ReflectMethod:
            class_name, method_name, *parameter_types = args
            cls = ctx.types[class_name]
            return cls.get_method(method_name, *(ctx.types[param_type] for param_type in parameter_types))

    class ConstructorCache(MemberCache):

        def get(self, ctx: ScriptingContext) -> ReflectConstructor:
            return super().get(ctx)

        def _provide(self, ctx, *args) -> ReflectConstructor:
            class_name, *parameter_types = args
            cls = ctx.types[class_name]
            return cls.get_constructor(*(ctx.types[param_type] for param_type in parameter_types))

    # Java STD

    class Wrapper:

        __slots__ = "_raw"

        def __init__(self, raw: ReflectObject):
            if raw is None:
                raise ValueError("Raw object is null.")
            self._raw = raw

        @property
        def raw(self) -> ReflectObject:
            return self._raw

        @property
        def context(self) -> ScriptingContext:
            return self._raw.context

        def __str__(self):
            if hasattr(self, "CLASS_NAME"):
                return "<Wrapped {}>".format(self.CLASS_NAME)
            else:
                return super().__str__()

    class Object(Wrapper):
        CLASS_NAME = "java.lang.Object"

    class String(Wrapper):
        CLASS_NAME = "java.lang.String"

    class BaseEnum(Wrapper):

        CLASS_NAME = "java.lang.Enum"
        METHOD_NAME = MethodCache(lambda ctx: (BaseEnum, "name"))
        METHOD_ORDINAL = MethodCache(lambda ctx: (BaseEnum, "ordinal"))

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            self._name: Optional[str] = None
            self._ordinal: Optional[int] = None

        @property
        def name(self) -> str:
            if self._name is None:
                self._name = self.METHOD_NAME.get(self.context)(self._raw)
            return self._name

        @property
        def ordinal(self) -> int:
            if self._ordinal is None:
                self._ordinal = self.METHOD_ORDINAL.get(self.context)(self._raw)
            return self._ordinal

    class BaseList(Wrapper):

        CLASS_NAME = "java.util.List"
        METHOD_SIZE = MethodCache(lambda: (BaseList, "size"))
        METHOD_ITERATOR = MethodCache(lambda: (BaseList, "iterator"))
        METHOD_GET = MethodCache(lambda: (BaseList, "get", "int"))

        def __init__(self, raw: ReflectObject, wrapper: Callable[[AnyReflectType], Any]):
            super().__init__(raw)
            self._wrapper = wrapper

        def __len__(self):
            return self.METHOD_SIZE.get(self.context)(self._raw)

        def __iter__(self):
            return BaseIterator(self.METHOD_ITERATOR.get(self.context)(self._raw), self._wrapper)

        def __getitem__(self, item):
            if isinstance(item, int):
                return self._wrapper(self.METHOD_GET.get(self.context)(self._raw))
            else:
                raise IndexError("list index out of range")

    class BaseIterator(Wrapper):

        CLASS_NAME = "java.util.Iterator"
        METHOD_HAS_NEXT = MethodCache(lambda: (BaseIterator, "hasNext"))
        METHOD_NEXT = MethodCache(lambda: (BaseIterator, "next"))

        def __init__(self, raw: ReflectObject, wrapper: Callable[[AnyReflectType], Any]):
            super().__init__(raw)
            self._wrapper = wrapper

        def __iter__(self):
            return self

        def __next__(self):
            if self.METHOD_HAS_NEXT.get(self.context)(self._raw):
                return self._wrapper(self.METHOD_NEXT.get(self.context)(self._raw))
            else:
                raise StopIteration

    # Minecraft STD

    class Minecraft(Wrapper):

        CLASS_NAME = "djz"

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            class_minecraft = raw.context.types[Minecraft]
            self._field_player = class_minecraft.get_field("s", raw.context.types[LocalPlayer]) # player
            self._field_level = class_minecraft.get_field("r", raw.context.types[ClientLevel]) # level
            self._gui: Optional[Gui] = None

        @classmethod
        def get_instance(cls, ctx: 'ScriptingContext') -> 'Minecraft':
            class_minecraft = ctx.types[Minecraft]
            field_instance = class_minecraft.get_field("F", class_minecraft)  # instance
            return Minecraft(field_instance.get_static())

        @property
        def player(self) -> 'Optional[LocalPlayer]':
            raw = self._field_player.get(self._raw)
            return None if raw is None else LocalPlayer(raw)

        @property
        def level(self) -> 'Optional[ClientLevel]':
            raw = self._field_level.get(self._raw)
            return None if raw is None else ClientLevel(raw)

        @property
        def gui(self) -> 'Gui':
            if self._gui is None:
                # This field is final in Minecraft's code, so we can cache it.
                class_minecraft = self._raw.context.types[Minecraft]
                field_gui = class_minecraft.get_field("j", self._raw.context.types[Gui]) # gui
                self._gui = Gui(field_gui.get(self._raw))
            return self._gui

        def __str__(self) -> str:
            return "<Minecraft>"

    class Component(Wrapper):

        CLASS_NAME = "nr"
        METHOD_GET_STRING = MethodCache(lambda: (Component, "getString"))

        @classmethod
        def ensure_component(cls, ctx: ScriptingContext, comp: Any) -> 'Component':
            if isinstance(comp, Component):
                return comp
            else:
                return TextComponent.new(ctx, str(comp))

        def get_string(self) -> str:
            return self.METHOD_GET_STRING.get(self.context).invoke(self._raw)

        def __str__(self):
            return "<{} '{}'>".format(self.__class__.__name__, self.get_string())

    class TextComponent(Component):

        CLASS_NAME = "oe"
        CONSTRUCTOR = ConstructorCache(lambda: (TextComponent, String))

        @classmethod
        def new(cls, ctx: ScriptingContext, text: str) -> 'TextComponent':
            return TextComponent(cls.CONSTRUCTOR.get(ctx).construct(text))

    class EntityPose(Enum):

        STANDING = 0
        FALL_FLYING = 1
        SLEEPING = 2
        SWIMMING = 3
        SPIN_ATTACK = 4
        CROUCHING = 5
        DYING = 6

    class Entity(Wrapper):

        CLASS_NAME = "aqa"  # Entity

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            class_entity = raw.context.types[Entity]
            self._field_x = class_entity.get_field("m", raw.context.types.double) # xo
            self._field_y = class_entity.get_field("n", raw.context.types.double) # yo
            self._field_z = class_entity.get_field("o", raw.context.types.double) # zo
            self._method_get_pose = class_entity.get_method("ae") # getPose
            self._method_get_name = class_entity.get_method("R") # getName
            self._method_get_type_name = class_entity.get_method("bJ") # getTypeName

        @property
        def x(self) -> float:
            return self._field_x.get(self._raw)

        @property
        def y(self) -> float:
            return self._field_y.get(self._raw)

        @property
        def z(self) -> float:
            return self._field_z.get(self._raw)

        @property
        def pose(self) -> EntityPose:
            raw_enum = self._method_get_pose.invoke(self._raw)
            if raw_enum is None:
                return EntityPose.STANDING
            else:
                try:
                    return EntityPose[BaseEnum(raw_enum).name]
                except KeyError:
                    return EntityPose.STANDING

        @property
        def name(self) -> Component:
            return Component(self._method_get_name.invoke(self._raw))

        @property
        def type_name(self) -> Component:
            return Component(self._method_get_type_name.invoke(self._raw))

        def __str__(self):
            return "<{}>".format(self.__class__.__name__)

    class LivingEntity(Entity):
        CLASS_NAME = "aqm"

    class Player(LivingEntity):
        CLASS_NAME = "bfw"

    class AbstractClientPlayer(Player):
        CLASS_NAME = "dzj"

    class LocalPlayer(Player):
        CLASS_NAME = "dzm"

    class LevelData:
        CLASS_NAME = "cyd"

    class WritableLevelData:
        CLASS_NAME = "cyo"

    class Level(Wrapper):

        CLASS_NAME = "brx"
        FIELD_LEVEL_DATA = FieldCache(lambda: (Level, "u", WritableLevelData)) # fieldData
        MATHOD_GET_GAME_TIME = MethodCache(lambda: (LevelData, "e")) # getGameTime()
        METHOD_GET_DAY_TIME = MethodCache(lambda: (LevelData, "f")) # getDayTime()
        METHOD_IS_RAINING = MethodCache(lambda: (LevelData, "k")) # isRaining()
        METHOD_IS_THUNDERING = MethodCache(lambda: (LevelData, "i")) # isThundering()

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            self._level_data = self.FIELD_LEVEL_DATA.get(raw.context).get(raw)

        @property
        def game_time(self) -> int:
            return self.MATHOD_GET_GAME_TIME.get(self.context).invoke(self._level_data)

        @property
        def day_time(self) -> int:
            return self.METHOD_GET_DAY_TIME.get(self.context)(self._level_data)

        @property
        def is_raining(self) -> int:
            return self.METHOD_IS_RAINING.get(self.context)(self._level_data)

        @property
        def is_thundering(self) -> int:
            return self.METHOD_IS_THUNDERING.get(self.context)(self._level_data)

    class ClientLevel(Level):

        CLASS_NAME = "dwt"
        METHOD_GET_PLAYERS = MethodCache(lambda: (ClientLevel, "x")) # players()

        def get_players(self) -> 'BaseList':
            return BaseList(self.METHOD_GET_PLAYERS.get(self.context).invoke(self._raw), AbstractClientPlayer)

    class Gui(Wrapper):

        CLASS_NAME = "dkv"
        METHOD_SET_OVERLAY_MESSAGE = MethodCache(lambda: (Gui, "a", Component, "boolean")) # setOverlayMessage
        METHOD_SET_TITLES = MethodCache(lambda: (Gui, "a", Component, Component, "int", "int", "int")) # setTitles

        def set_overlay(self, comp: Any, animate_color: bool = False):
            comp = Component.ensure_component(self.context, comp)
            self.METHOD_SET_OVERLAY_MESSAGE.get(self.context)(self._raw, comp.raw, animate_color)

        def set_title(self, *, title: Optional[Any] = None, subtitle: Optional[Any] = None, fade_in: int = -1, stay: int = -1, fade_out: int = -1):

            if fade_in != -1 or stay != -1 or fade_out != -1:
                self.METHOD_SET_TITLES.get(self.context)(self._raw, None, None, fade_in, stay, fade_out)

            if title is not None:
                title = Component.ensure_component(self.context, title)
                self.METHOD_SET_TITLES.get(self.context)(self._raw, title.raw, None, -1, -1, -1)

            if subtitle is not None:
                subtitle = Component.ensure_component(self.context, subtitle)
                self.METHOD_SET_TITLES.get(self.context)(self._raw, None, subtitle.raw, -1, -1, -1)

    # Byte buffer utils

    class ByteBuffer:

        def __init__(self, size: int):
            self.data = bytearray(size)
            self.limit = 0
            self.pos = 0

        def clear(self):
            self.pos = 0
            self.limit = len(self.data)

        def remaining(self) -> int:
            return self.limit - self.pos

        def lshift(self, count: int):
            self.data[:(len(self.data) - count)] = self.data[count:]

        def ensure_len(self, length: int, offset: Optional[int] = None):
            real_offset = self.pos if offset is None else offset
            if real_offset + length > self.limit:
                raise ValueError("No more space in the buffer (pos: {}, limit: {}).".format(self.pos, self.limit))
            else:
                if offset is None:
                    self.pos += length
                return real_offset

        # PUT #

        def put(self, byte: int, *, offset=None):
            struct.pack_into(">B", self.data, self.ensure_len(1, offset), byte & 0xFF)

        def put_bytes(self, arr: Union[bytes, bytearray], length=None, *, offset=None):
            if length is None:
                length = len(arr)
            pos = self.ensure_len(length, offset)
            self.data[pos:(pos + length)] = arr[:length]

        def put_short(self, short: int, *, offset=None):
            struct.pack_into(">H", self.data, self.ensure_len(2, offset), short & 0xFFFF)

        def put_int(self, integer: int, *, offset=None):
            struct.pack_into(">I", self.data, self.ensure_len(4, offset), integer & 0xFFFFFFFF)

        def put_long(self, long: int, *, offset=None):
            struct.pack_into(">Q", self.data, self.ensure_len(8, offset), long & 0xFFFFFFFFFFFFFFFF)

        def put_float(self, flt: float, *, offset=None):
            struct.pack_into(">f", self.data, self.ensure_len(4, offset), flt)

        def put_double(self, dbl: float, *, offset=None):
            struct.pack_into(">d", self.data, self.ensure_len(8, offset), dbl)

        def put_char(self, char: str, *, offset=None):
            self.put_short(ord(char[0]), offset=offset)

        def put_string(self, string: str, *, offset=None):
            str_buf = string.encode()
            str_buf_len = len(str_buf)
            offset = self.ensure_len(2 + str_buf_len, offset)
            self.put_short(str_buf_len, offset=offset)
            self.data[(offset + 2):(offset + 2 + str_buf_len)] = str_buf

        # GET #

        def get(self, *, offset=None, signed=True) -> int:
            return struct.unpack_from(">b" if signed else ">B", self.data, self.ensure_len(1, offset))[0]

        def get_short(self, *, offset=None, signed=True) -> int:
            return struct.unpack_from(">h" if signed else ">H", self.data, self.ensure_len(2, offset))[0]

        def get_int(self, *, offset=None, signed=True) -> int:
            return struct.unpack_from(">i" if signed else ">I", self.data, self.ensure_len(4, offset))[0]

        def get_long(self, *, offset=None, signed=True) -> int:
            return struct.unpack_from(">q" if signed else ">Q", self.data, self.ensure_len(8, offset))[0]

        def get_float(self, *, offset=None) -> int:
            return struct.unpack_from(">f", self.data, self.ensure_len(4, offset))[0]

        def get_double(self, *, offset=None) -> int:
            return struct.unpack_from(">d", self.data, self.ensure_len(8, offset))[0]

        def get_char(self, *, offset=None) -> str:
            return chr(self.get_short(offset=offset, signed=False))

        def get_string(self, *, offset=None) -> str:
            str_len = self.get_short(offset=offset, signed=False)
            str_pos = self.ensure_len(str_len)
            return self.data[str_pos:(str_pos + str_len)].decode()

    return ScriptingAddon(pmc_in)
