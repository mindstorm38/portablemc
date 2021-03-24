from ..reflect import Object, Wrapper, AnyTypeWrapped, MethodCache
from typing import Callable, Optional


__all__ = [
    "Iterable", "Iterator",
    "Collection", "List", "Queue",
    "List"
]


class Iterable(Wrapper):

    class_name = "java.lang.Iterable"
    method_iterator = MethodCache(lambda: (Iterable, "iterator"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[Object], Wrapper]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __iter__(self):
        return Iterator(self.method_iterator.invoke(self._raw), self._wrapper)

    def _wrap_optional(self, val: 'Optional[Object]') -> 'Optional[Wrapper]':
        return None if val is None else self._wrapper(val)


class Iterator(Wrapper):

    class_name = "java.util.Iterator"
    method_has_next = MethodCache(lambda: (Iterator, "hasNext"))
    method_next = MethodCache(lambda: (Iterator, "next"))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[Object], Wrapper]'):
        super().__init__(raw)
        self._wrapper = wrapper

    def __iter__(self):
        return self

    def __next__(self) -> 'Wrapper':
        if self.method_has_next.invoke(self._raw):
            return self._wrapper(self.method_next.invoke(self._raw))
        else:
            raise StopIteration


class Collection(Iterable):

    class_name = "java.util.Collection"
    method_size = MethodCache(lambda: (Collection, "size"))
    method_contains = MethodCache(lambda: (Collection, "contains", Wrapper))
    method_add = MethodCache(lambda: (Collection, "add", Wrapper))
    method_remove = MethodCache(lambda: (Collection, "remove", Wrapper))
    method_contains_all = MethodCache(lambda: (Collection, "containsAll", Collection))
    method_add_all = MethodCache(lambda: (Collection, "addAll", Collection))
    method_remove_all = MethodCache(lambda: (Collection, "removeAll", Collection))
    method_clear = MethodCache(lambda: (Collection, "clear"))
    __slots__ = ()

    def __len__(self):
        return self.method_size.invoke(self._raw)

    def __contains__(self, o: AnyTypeWrapped):
        return self.method_contains.invoke(self._raw, Wrapper.ensure_object(o))

    def add(self, e: AnyTypeWrapped) -> bool:
        return self.method_add.invoke(self._raw, Wrapper.ensure_object(e))

    def remove(self, e: AnyTypeWrapped) -> bool:
        return self.method_remove.invoke(self._raw, Wrapper.ensure_object(e))

    def contains_all(self, other: 'Collection') -> bool:
        return self.method_contains_all.invoke(self._raw, other.raw)

    def add_all(self, other: 'Collection') -> bool:
        return self.method_add_all.invoke(self._raw, other.raw)

    def remove_all(self, other: 'Collection') -> bool:
        return self.method_remove_all.invoke(self._raw, other.raw)

    def clear(self):
        self.method_clear.invoke(self._raw)


class List(Collection):

    class_name = "java.util.List"
    method_get = MethodCache(lambda: (List, "get", "int"))
    method_set = MethodCache(lambda: (List, "set", "int", Wrapper))
    method_add_at = MethodCache(lambda: (List, "add", "int", Wrapper))
    method_remove_at = MethodCache(lambda: (List, "remove", "int"))
    method_index_of = MethodCache(lambda: (List, "indexOf", Wrapper))
    method_last_index_of = MethodCache(lambda: (List, "lastIndexOf", Wrapper))
    __slots__ = "_wrapper"

    def __init__(self, raw: 'Object', wrapper: 'Callable[[Object], Wrapper]'):
        super().__init__(raw, wrapper)

    def get(self, index: int) -> 'Optional[Wrapper]':
        return self._wrapper(self.method_get.invoke(self._raw, index))

    def set(self, index: int, obj: 'AnyTypeWrapped') -> 'Optional[Wrapper]':
        return self._wrap_optional(self.method_set.invoke(self._raw, index, Wrapper.ensure_object(obj)))

    def add_at(self, index: int, obj: 'AnyTypeWrapped'):
        self.method_add_at.invoke(self._raw, index, Wrapper.ensure_object(obj))

    def remove_at(self, index: int) -> 'Optional[Wrapper]':
        return self._wrap_optional(self.method_remove_at.invoke(self._raw, index))

    def index_of(self, obj: 'AnyTypeWrapped') -> int:
        return self.method_index_of.invoke(self._raw, Wrapper.ensure_object(obj))

    def last_index_of(self, obj: 'AnyTypeWrapped') -> int:
        return self.method_last_index_of.invoke(self._raw, Wrapper.ensure_object(obj))

    def __getitem__(self, item) -> 'Optional[Wrapper]':
        if isinstance(item, int):
            return self._wrap_optional(self.method_get.invoke(self._raw))
        else:
            raise IndexError("list index out of range")

    def __setitem__(self, key, value):
        if isinstance(key, int):
            self.method_set.invoke(self._raw, Wrapper.ensure_object(value))
        else:
            raise IndexError("list index out of range")


class Queue(Collection):

    class_name = "java.util.Queue"

    method_add_first = MethodCache(lambda: (Queue, "add", Wrapper))
    method_offer_first = MethodCache(lambda: (Queue, "offer", Wrapper))

    method_remove_first = MethodCache(lambda: (Queue, "remove"))
    method_poll_first = MethodCache(lambda: (Queue, "poll"))

    method_get_first = MethodCache(lambda: (Queue, "element"))
    method_peek_first = MethodCache(lambda: (Queue, "peek"))

    __slots__ = ()

    def add_first(self, e: 'AnyTypeWrapped') -> bool:
        """ Insert element at the front, throw IllegalStateException if not possible. """
        return self.method_add_first.invoke(self._raw, Wrapper.ensure_object(e))

    def offer_first(self, e: 'AnyTypeWrapped') -> bool:
        """ Insert element at the front, return True if possible. """
        return self.method_offer_first.invoke(self._raw, Wrapper.ensure_object(e))

    def remove_first(self) -> 'Optional[Wrapper]':
        """ Remove and retreive the first element, throw NoSuchElementException if empty. """
        return self._wrap_optional(self.method_remove_first.invoke(self._raw))

    def poll_first(self) -> 'Optional[Wrapper]':
        """ Remove and retreive the first element, return None if empty. """
        return self._wrap_optional(self.method_poll_first.invoke(self._raw))

    def get_first(self) -> 'Optional[Wrapper]':
        """ Retreive the first element, throw NoSuchElementException if empty. """
        return self._wrap_optional(self.method_get_first.invoke(self._raw))

    def peek_first(self) -> 'Optional[Wrapper]':
        """ Retreive the first element, return None if empty. """
        return self._wrap_optional(self.method_peek_first.invoke(self._raw))
