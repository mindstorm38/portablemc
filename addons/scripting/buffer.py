from typing import Optional, Union
import struct


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
