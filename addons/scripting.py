
NAME = "Scripting"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "addon:richer", "prompt_toolkit"


# Candidate client tick hooks for every version:
# - 1.14.4 - 1.16 'Queue<Runnable> Minecraft.progressTasks' (or 'Minecraft.tell(Runnable)')


def addon_build():

    from prompt_toolkit.key_binding.bindings.focus import focus_next, focus_previous
    from prompt_toolkit.layout.containers import Window, HSplit, VSplit, Container
    from prompt_toolkit.layout.controls import FormattedTextControl
    from prompt_toolkit.completion import CompleteEvent, Completion
    from prompt_toolkit.key_binding import KeyBindings
    from prompt_toolkit.application import Application
    from prompt_toolkit.completion import Completer
    from prompt_toolkit.document import Document
    from prompt_toolkit.widgets import TextArea
    from prompt_toolkit.buffer import Buffer

    from typing import List, Callable, Optional, Union, Tuple, Iterable
    from argparse import ArgumentParser, Namespace
    from threading import Thread
    from os import path
    import socket
    import struct
    import time


    TEMP_JAR_FILE_PATH = path.join(path.dirname(__file__), "scripting_dev/out/artifacts/portablemc_scripting_dev_jar/portablemc_scripting_dev.jar")

    PACKET_GET_CLASS = 1
    PACKET_GET_FIELD = 2
    PACKET_GET_METHOD = 3
    PACKET_FIELD_GET = 10
    PACKET_FIELD_SET = 11
    PACKET_METHOD_INVOKE = 20
    PACKET_RESULT = 30


    class ScriptingAddon:

        def __init__(self, pmc):

            self.pmc = pmc
            self.richer = None

            self.server: 'Optional[ScriptingServer]' = None
            self.active = False

            self.interpreter: 'Optional[Interpreter]' = None
            self.interpreter_window = None
            self.interpreter_input: 'Optional[InterpreterInput]' = None

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

            if self.active:

                title_text = self.pmc.get_message("start.scripting.title", self.server.get_port())

                def interpreter_output_callback(text: str):
                    self.interpreter_window.append(*text.splitlines(keepends=True))

                self.interpreter = Interpreter(self.server.get_context(), interpreter_output_callback)
                self.interpreter_window = self.richer.LimitedBufferWindow(100, wrap_lines=True)
                self.interpreter_input = InterpreterInput(self.interpreter)

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
                            HSplit([
                                self.interpreter_window,
                                self.interpreter_input
                            ]),
                            Window(width=1)
                        ])
                    ])
                ])

                keys.add("tab")(focus_next)
                keys.add("s-tab")(focus_previous)

            app = old(container, keys)

            if self.active:
                app.layout.focus(self.interpreter_input)

            return app

    class Interpreter:

        def __init__(self, context: 'ScriptingContext', output_callback: Callable[[str], None]):

            builtins = dict(globals()["__builtins__"])
            builtins["print"] = self.custom_print
            del builtins["help"]
            del builtins["input"]
            del builtins["breakpoint"]

            self.globals = {
                "ctx": context,
                "ty": context.types,
                "__builtins__": builtins
            }
            self.locals = {}
            self.output_callback = output_callback

        def custom_print(self, *args, sep: str = " ", end: str = "\n", **_kwargs):
            self.output_callback("{}{}".format(sep.join(str(arg) for arg in args), end))

        def _out_traceback(self):
            import traceback
            import sys
            err_type, err, tb = sys.exc_info()
            for line in traceback.extract_tb(tb).format()[2:]:
                self.output_callback("{}".format(line))
            self.output_callback("{}: {}\n".format(err_type.__name__, err))

        def interpret(self, text: str):
            self.output_callback(">>> {}\n".format(text))
            if len(text):
                try:
                    ret = eval(text, self.globals, self.locals)
                    self.output_callback("{}\n".format(ret))
                except SyntaxError:
                    try:
                        exec(text, self.globals, self.locals)
                    except (BaseException,):
                        self._out_traceback()
                except (BaseException,):
                    self._out_traceback()

    class InterpreterInput:

        def __init__(self, interpreter: 'Interpreter'):
            self.input = TextArea(
                height=1,
                prompt=">>> ",
                multiline=False,
                wrap_lines=False,
                completer=InterpreterCompleter(interpreter)
            )
            self.interpreter = interpreter
            self.input.accept_handler = self._accept

        def _accept(self, buffer: Buffer):
            Thread(target=lambda: self.interpreter.interpret(buffer.text), daemon=True).start()

        def __pt_container__(self):
            return self.input

    class InterpreterCompleter(Completer):

        def __init__(self, interpreter: 'Interpreter'):
            self.interpreter = interpreter

        def get_completions(self, document: Document, complete_event: CompleteEvent) -> Iterable[Completion]:
            return []

    # TCP server and reflected objects definitions

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

        def send_get_method_packet(self, owner: 'ReflectClass', method_name: str, parameter_types: Tuple['ReflectClass', ...]):
            self._prepare_packet()
            self._tx_buf.put_int(owner.internal_index)
            self._tx_buf.put_string(method_name)
            self._tx_buf.put(len(parameter_types))
            for ptype in parameter_types:
                self._tx_buf.put_int(ptype.internal_index)
            self._send_packet(PACKET_GET_METHOD)
            idx = self._wait_for_packet(PACKET_RESULT).get_int()
            return None if idx == -1 else ReflectMethod(self._context, idx, owner, method_name, parameter_types)

        def send_field_get_packet(self, field: 'ReflectField', owner: Optional['ReflectObject']) -> 'AnyReflectType':
            self._prepare_packet()
            self._tx_buf.put_int(field.internal_index)
            self._tx_buf.put_int(-1 if owner is None else owner.internal_index)
            self._send_packet(PACKET_FIELD_GET)
            return self._get_value(self._wait_for_packet(PACKET_RESULT))

        def send_field_set_packet(self, field: 'ReflectField', owner: Optional['ReflectObject'], val: 'AnyReflectType'):
            self._prepare_packet()
            self._tx_buf.put_int(field.internal_index)
            self._tx_buf.put_int(-1 if owner is None else owner.internal_index)
            self._put_value(self._tx_buf, val, field.get_type())
            self._send_packet(PACKET_FIELD_SET)
            self._wait_for_packet(PACKET_RESULT)

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
            elif isinstance(val, int):
                data = self._put_value_int_encoders.get(target_type.get_name())
                if data is None:
                    raise ValueError(
                        "Integer value {} is not suitable for {} type.".format(val, target_type.get_name()))
                buf.put_int(data[0])
                (data[1])(buf, val)
            elif isinstance(val, bool):
                if target_type.get_name() != "boolean":
                    raise ValueError("Boolean {} given but expected {}.".format(val, target_type.get_name()))
                buf.put_int(-11 if val else -10)
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
            self._types = TypesCache(server)
            self._minecraft: 'Optional[Minecraft]' = None

        @property
        def server(self) -> 'ScriptingServer':
            return self._server

        @property
        def types(self) -> 'TypesCache':
            return self._types

        @property
        def minecraft(self) -> 'Minecraft':
            if self._minecraft is None:
                self._minecraft = Minecraft(self)
            return self._minecraft

        def __str__(self):
            return "<ScriptingContext>"

    class TypesCache:

        def __init__(self, server: ScriptingServer):
            self._server = server
            self._types = {}

        def _get(self, name: str) -> 'Optional[ReflectClass]':
            typ = self._types.get(name)
            if typ is None:
                typ = self._types[name] = self._server.send_get_class_packet(name)
                if typ is None:
                    raise ClassNotFoundError("Class '{}' not found.".format(name))
            return typ

        def __getattr__(self, item: str):
            return self._get(item)

        def __getitem__(self, item: str):
            return self._get(item)

        def __str__(self):
            return "<TypesCache>"

    class ReflectObject:

        __slots__ = "_ctx", "_idx"

        def __init__(self, ctx: ScriptingContext, idx: int):
            self._ctx = ctx
            self._idx = idx

        @property
        def context(self) -> 'ScriptingContext':
            return self._ctx

        @property
        def internal_index(self) -> int:
            return self._idx

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

        def get_field(self, name: str, field_type: 'ReflectClass') -> 'Optional[ReflectField]':
            return self._ctx.server.send_get_field_packet(self, name, field_type)

        def get_method(self, name: str, parameter_types: Tuple['ReflectClass', ...]) -> 'Optional[ReflectMethod]':
            return self._ctx.server.send_get_method_packet(self, name, parameter_types)

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
            return self._ctx.server.send_field_get_packet(self, owner)

        def get_static(self) -> AnyReflectType:
            return self.get(None)

        def set(self, owner: Optional[ReflectObject], val: AnyReflectType):
            self._ctx.server.send_field_set_packet(self, owner, val)

        def set_static(self, val: AnyReflectType):
            self.set(None, val)

        def __str__(self):
            return "<Field {} {}.{}>".format(self._type.get_name(), self._owner.get_name(), self._name)

    class ReflectMethod(ReflectClassMember):

        __slots__ = "_parameter_types"

        def __init__(self, ctx: ScriptingContext, idx: int, owner: ReflectClass, name: str, parameter_types: Tuple['ReflectClass', ...]):
            super().__init__(ctx, idx, owner, name)
            self._parameter_types = parameter_types

        def __str__(self):
            return "<Method {}.{}({})>".format(
                self._owner.get_name(),
                self._name,
                ", ".format(*(typ.get_name for typ in self._parameter_types))
            )

    class ClassNotFoundError(Exception):
        pass

    # Advanved class wrappers

    class Minecraft:

        def __init__(self, ctx: ScriptingContext):
            self._class_minecraft = ctx.types["djz"]  # Minecraft
            field_instance = self._class_minecraft.get_field("F", self._class_minecraft)  # instance
            self._raw = field_instance.get_static()
            class_local_player = ctx.types[LocalPlayer.CLASS_LOCAL_PLAYER]
            self._field_player = self._class_minecraft.get_field("s", class_local_player)  # player

        @property
        def player(self) -> 'Optional[LocalPlayer]':
            raw = self._field_player.get(self._raw)
            return None if raw is None else LocalPlayer(raw)

        def __str__(self) -> str:
            return "<Minecraft>"

    class Entity:

        CLASS_ENTITY = "aqa"  # Entity

        def __init__(self, raw: ReflectObject):
            self._raw = raw
            self._class_entity = raw.context.types[self.CLASS_ENTITY]
            self._field_x = self._class_entity.get_field("m", raw.context.types.double)  # xo
            self._field_y = self._class_entity.get_field("n", raw.context.types.double)  # yo
            self._field_z = self._class_entity.get_field("o", raw.context.types.double)  # zo

        @property
        def x(self) -> float:
            return self._field_x.get(self._raw)

        @property
        def y(self) -> float:
            return self._field_y.get(self._raw)

        @property
        def z(self) -> float:
            return self._field_z.get(self._raw)

        def __str__(self):
            return "<{}>".format(self.__class__.__name__)

    class LivingEntity(Entity):

        CLASS_LIVING_ENTITY = "aqm"  # LivingEntity

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            self._class_living_entity = raw.context.types[self.CLASS_LIVING_ENTITY]

    class Player(LivingEntity):

        CLASS_PLAYER = "bfw"  # Player

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            self._class_player = raw.context.types[self.CLASS_PLAYER]

    class LocalPlayer(Player):

        CLASS_LOCAL_PLAYER = "dzm"  # LocalPlayer

        def __init__(self, raw: ReflectObject):
            super().__init__(raw)
            self._class_local_player = raw.context.types[self.CLASS_LOCAL_PLAYER]

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

    return ScriptingAddon
