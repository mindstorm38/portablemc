from typing import Callable, Any
from ..reflect import Object, MethodCache, AnyType
from .lang import ObjectWrapper


__all__ = ["BaseList", "BaseIterator"]


METHOD_SIZE = MethodCache(lambda: (BaseList, "size"))
METHOD_ITERATOR = MethodCache(lambda: (BaseList, "iterator"))
METHOD_GET = MethodCache(lambda: (BaseList, "get", "int"))
METHOD_HAS_NEXT = MethodCache(lambda: (BaseIterator, "hasNext"))
METHOD_NEXT = MethodCache(lambda: (BaseIterator, "next"))


class BaseList(ObjectWrapper):

    type_name = "java.util.List"
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __len__(self):
        METHOD_SIZE.get(self.runtime).invoke(self._raw)

    def __iter__(self):
        return BaseIterator(METHOD_ITERATOR.get(self.runtime)(self._raw), self._wrapper)

    def __getitem__(self, item):
        if isinstance(item, int):
            return self._wrapper(METHOD_GET.get(self.runtime)(self._raw))
        else:
            raise IndexError("list index out of range")


class BaseIterator(ObjectWrapper):

    type_name = "java.util.Iterator"
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[AnyType], Any]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __iter__(self):
        return self

    def __next__(self):
        if METHOD_HAS_NEXT.get(self.runtime)(self._raw):
            return self._wrapper(METHOD_NEXT.get(self.runtime)(self._raw))
        else:
            raise StopIteration
