from typing import Callable, Any
from ..reflect import Object, AnyType, Wrapper, MethodCache


__all__ = ["List", "Iterator"]


class List(Wrapper):

    type_name = "java.util.List"
    method_size = MethodCache(lambda: (List, "size"))
    method_iterator = MethodCache(lambda: (List, "iterator"))
    method_get = MethodCache(lambda: (List, "get", "int"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __len__(self):
        self.method_size.invoke(self._raw)

    def __iter__(self):
        return Iterator(self.method_iterator.invoke(self._raw), self._wrapper)

    def __getitem__(self, item):
        if isinstance(item, int):
            return self._wrapper(self.method_get.invoke(self._raw))
        else:
            raise IndexError("list index out of range")


class Iterator(Wrapper):

    type_name = "java.util.Iterator"
    method_has_next = MethodCache(lambda: (Iterator, "hasNext"))
    method_next = MethodCache(lambda: (Iterator, "next"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __iter__(self):
        return self

    def __next__(self):
        if self.method_has_next.invoke(self._raw):
            return self._wrapper(self.method_next.invoke(self._raw))
        else:
            raise StopIteration
