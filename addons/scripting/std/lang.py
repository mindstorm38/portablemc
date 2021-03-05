from ..reflect import Wrapper, Object, MethodCache
from typing import Optional


__all__ = ["String", "Enum"]


class String(Wrapper):
    type_name = "java.lang.String"


class Enum(Wrapper):

    type_name = "java.lang.Enum"
    method_name = MethodCache(lambda ctx: (Enum, "name"))
    method_ordinal = MethodCache(lambda ctx: (Enum, "ordinal"))
    __slots__ = "_name", "_ordinal"

    def __init__(self, raw: 'Object'):
        super().__init__(raw)
        self._name: Optional[str] = None
        self._ordinal: Optional[int] = None

    @property
    def name(self) -> str:
        if self._name is None:
            self._name = self.method_name.invoke(self._raw)
        return self._name

    @property
    def ordinal(self) -> int:
        if self._ordinal is None:
            self._ordinal = self.method_ordinal.invoke(self._raw)
        return self._ordinal
