from typing import  Optional, Tuple, Dict, Callable
from threading import Thread, Event
import socket

from .reflect import Runtime, Object, Class, Field, Executable, Method, Constructor, \
    AnyType, ClassNotFoundError, FieldNotFoundError, MethodNotFoundError
from .buffer import ByteBuffer


PACKET_GET_CLASS = 1
PACKET_GET_FIELD = 2
PACKET_GET_METHOD = 3

PACKET_FIELD_GET = 10
PACKET_FIELD_SET = 11

PACKET_METHOD_INVOKE = 20

PACKET_OBJECT_GET_CLASS = 30
PACKET_OBJECT_IS_INSTANCE = 31

PACKET_BIND_CALLBACK = 40

PACKET_RESULT = 100
PACKET_RESULT_CLASS = 101
PACKET_RESULT_BYTE = 102

PACKET_GENERIC_ERROR = 110


class ScriptingServer(Runtime):

    def __init__(self):

        super().__init__()

        self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self._stop_event = Event()
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

        self._classes_cache: 'Dict[str, Class]' = {}

    def start(self):

        self._socket.bind(('127.0.0.1', 0))
        self._port = self._socket.getsockname()[1]

        thread = Thread(target=self._entry, name="PortableMC Scripting Server Thread", daemon=True)
        thread.start()

    def stop(self):
        self._stop_event.set()

    def get_port(self) -> int:
        return self._port

    def _entry(self):
        self._socket.listen(1)
        self._client_socket, _ = self._socket.accept()
        self._stop_event.wait()
        self._socket.close()

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
                    if packet_type == PACKET_GENERIC_ERROR:
                        raise ValueError(f"Server generic error: {self._rx_buf.get_string()}")
                    else:
                        print("[SCRIPTING] Invalid received packet type, expected {}, got {}.".format(expected_packet_type, packet_type))
                    self._rx_buf.lshift(next_packet_len)
                    next_packet_len = 0
            else:
                remaining = self._rx_buf.remaining()
                read_len = self._client_socket.recv_into(self._rx_recv_buf, min(len(self._rx_recv_buf), remaining))
                self._rx_buf.put_bytes(self._rx_recv_buf, read_len)

    # Packets implementations

    def send_get_class_packet(self, name: str) -> int:
        self._prepare_packet()
        self._tx_buf.put_string(name)
        self._send_packet(PACKET_GET_CLASS)
        return self._wait_for_packet(PACKET_RESULT).get_int()

    def send_object_get_class_packet(self, obj: 'Object') -> Optional[Tuple[str, int]]:
        self._prepare_packet()
        self._tx_buf.put_int(obj.get_pointer())
        self._send_packet(PACKET_OBJECT_GET_CLASS)
        buf = self._wait_for_packet(PACKET_RESULT_CLASS)
        idx = buf.get_int()
        if idx == -1:
            return None
        else:
            return buf.get_string(), idx

    def send_object_is_instance_packet(self, cls: 'Class', obj: 'Object') -> bool:
        self._prepare_packet()
        self._tx_buf.put_int(cls.get_pointer())
        self._tx_buf.put_int(obj.get_pointer())
        self._send_packet(PACKET_OBJECT_IS_INSTANCE)
        buf = self._wait_for_packet(PACKET_RESULT_BYTE)
        return buf.get() != 0

    def send_get_field_packet(self, cls: 'Class', name: str, field_type: 'Class') -> int:
        self._prepare_packet()
        self._tx_buf.put_int(cls.get_pointer())
        self._tx_buf.put_string(name)
        self._tx_buf.put_int(field_type.get_pointer())
        self._send_packet(PACKET_GET_FIELD)
        return self._wait_for_packet(PACKET_RESULT).get_int()

    def send_get_method_packet(self, cls: 'Class', name: str, parameter_types: 'Tuple[Class, ...]') -> int:
        # Empty method name means we want a constructor
        self._prepare_packet()
        self._tx_buf.put_int(cls.get_pointer())
        self._tx_buf.put_string(name)
        self._tx_buf.put(len(parameter_types))
        for ptype in parameter_types:
            self._tx_buf.put_int(ptype.get_pointer())
        self._send_packet(PACKET_GET_METHOD)
        return self._wait_for_packet(PACKET_RESULT).get_int()

    def send_field_get_packet(self, field: 'Field', owner: 'Optional[Object]') -> 'AnyType':
        self._prepare_packet()
        self._tx_buf.put_int(field.get_pointer())
        self._tx_buf.put_int(-1 if owner is None else owner.get_pointer())
        self._send_packet(PACKET_FIELD_GET)
        return self._get_value(self._wait_for_packet(PACKET_RESULT))

    def send_field_set_packet(self, field: 'Field', owner: 'Optional[Object]', value: 'AnyType'):
        self._prepare_packet()
        self._tx_buf.put_int(field.get_pointer())
        self._tx_buf.put_int(-1 if owner is None else owner.get_pointer())
        self._put_value(self._tx_buf, value, field.get_type())
        self._send_packet(PACKET_FIELD_SET)
        self._wait_for_packet(PACKET_RESULT)

    def send_method_invoke_packet(self, executable: 'Executable', owner: 'Optional[Object]', parameters: 'Tuple[AnyType, ...]') -> 'AnyType':
        param_types = executable.get_parameter_types()
        if len(param_types) != len(parameters):
            raise ValueError(f"Parameters count doesn't match, got {len(parameters)}, expected {len(param_types)}.")
        self._prepare_packet()
        self._tx_buf.put_int(executable.get_pointer())
        self._tx_buf.put_int(-1 if owner is None else owner.get_pointer())
        self._tx_buf.put(len(parameters))
        for idx, param in enumerate(parameters):
            self._put_value(self._tx_buf, param, param_types[idx])
        self._send_packet(PACKET_METHOD_INVOKE)
        return self._get_value(self._wait_for_packet(PACKET_RESULT))

    def send_bind_callback_packet(self, cls: 'Class') -> 'Tuple[int, Object]':
        self._prepare_packet()
        self._tx_buf.put_int(cls.get_pointer())
        self._send_packet(PACKET_BIND_CALLBACK)
        # TODO

    # Decode reflect value

    def _get_value(self, buf: 'ByteBuffer') -> 'AnyType':
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
            return Object(self, idx)

    def _put_value(self, buf: 'ByteBuffer', val: 'AnyType', target_type: 'Class'):
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
            buf.put_int(val.get_pointer())

    # Runtime implementations

    def get_class_from_name(self, name: str) -> 'Class':
        cached = self._classes_cache.get(name)
        if cached is not None:
            return cached
        ptr = self.send_get_class_packet(name)
        if ptr < 0:
            raise ClassNotFoundError(f"Class '{name}' not found.")
        cached = Class(self, ptr, name)
        self._classes_cache[name] = cached
        return cached

    def get_class_from_object(self, obj: 'Object') -> 'Class':
        res = self.send_object_get_class_packet(obj)
        if res is None:
            raise ClassNotFoundError(f"Illegal object, class not found.")
        return Class(self, res[1], res[0])

    def get_class_field_from_name(self, cls: 'Class', name: str, field_type: 'Class') -> 'Field':
        ptr = self.send_get_field_packet(cls, name, field_type)
        if ptr < 0:
            raise FieldNotFoundError(f"Field '{name}' not found in class '{cls.get_name()}'.")
        return Field(self, ptr, cls, name, field_type)

    def get_class_method_from_name(self, cls: 'Class', name: str, parameter_types: 'Tuple[Class, ...]') -> 'Method':
        if not len(name):
            raise ValueError("Class name is empty.")
        ptr = self.send_get_method_packet(cls, name, parameter_types)
        if ptr < 0:
            raise MethodNotFoundError(f"Method '{name}' not found in class '{cls.get_name()}'.")
        return Method(self, ptr, cls, name, parameter_types)

    def get_class_constructor_from_name(self, cls: 'Class', parameter_types: 'Tuple[Class, ...]') -> 'Constructor':
        ptr = self.send_get_method_packet(cls, "", parameter_types)
        if ptr < 0:
            raise MethodNotFoundError(f"Constructor not found in class '{cls.get_name()}'.")
        return Constructor(self, ptr, cls, parameter_types)

    def get_field_value(self, field: 'Field', owner: 'Optional[Object]') -> 'AnyType':
        return self.send_field_get_packet(field, owner)

    def set_field_value(self, field: 'Field', owner: 'Optional[Object]', value: 'AnyType'):
        self.send_field_set_packet(field, owner, value)

    def invoke_method(self, method: 'Method', owner: 'Optional[Object]', parameters: 'Tuple[AnyType, ...]') -> 'AnyType':
        return self.send_method_invoke_packet(method, owner, parameters)

    def invoke_constructor(self, constructor: 'Constructor', parameters: 'Tuple[AnyType, ...]') -> 'Object':
        return self.send_method_invoke_packet(constructor, None, parameters)

    def is_class_instance(self, cls: 'Class', obj: 'Object') -> bool:
        return self.send_object_is_instance_packet(cls, obj)

    def bind_callback(self, cls: 'Class', func: 'Callable') -> 'Object':

        pass
