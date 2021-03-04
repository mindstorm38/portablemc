from typing import Callable, Any
from ..reflect import Object, AnyType, Wrapper, MethodCache


__all__ = ["BaseList", "BaseIterator"]


class BaseList(Wrapper):

    type_name = "java.util.List"
    method_size = MethodCache(lambda: (BaseList, "size"))
    method_iterator = MethodCache(lambda: (BaseList, "iterator"))
    method_get = MethodCache(lambda: (BaseList, "get", "int"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __len__(self):
        self.method_size.get(self.runtime).invoke(self._raw)

    def __iter__(self):
        return BaseIterator(self.method_iterator.get(self.runtime)(self._raw), self._wrapper)

    def __getitem__(self, item):
        if isinstance(item, int):
            return self._wrapper(self.method_get.get(self.runtime)(self._raw))
        else:
            raise IndexError("list index out of range")


class BaseIterator(Wrapper):

    type_name = "java.util.Iterator"
    method_has_next = MethodCache(lambda: (BaseIterator, "hasNext"))
    method_next = MethodCache(lambda: (BaseIterator, "next"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __iter__(self):
        return self

    def __next__(self):
        if self.method_has_next.get(self.runtime)(self._raw):
            return self._wrapper(self.method_next.get(self.runtime)(self._raw))
        else:
            raise StopIteration
