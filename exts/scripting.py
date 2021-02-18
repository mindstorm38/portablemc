from typing import Optional

NAME = "Scripting"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"


def load(portablemc):
    # raise Exception("test")
    pass


class ScriptingContext:

    def __init__(self):
        pass

    def get_minecraft(self):
        pass

    def get_class(self, full_path: str) -> Optional['ClassWrapper']:
        return None


class ClassWrapper:

    __slots__ = "_full_path",

    def __init__(self, full_path: str):
        self._full_path = full_path

    def construct(self) -> Optional['ObjectWrapper']:
        return None

    def call_static(self, name: str, *args) -> Optional['ObjectWrapper']:
        pass

    def get_static(self, name: str) -> Optional['ObjectWrapper']:
        pass


class ObjectWrapper:

    __slots__ = "_uid",

    def __init__(self, uid: int):
        self._uid = uid
