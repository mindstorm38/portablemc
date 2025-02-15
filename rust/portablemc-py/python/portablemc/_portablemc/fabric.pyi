from typing import Self
from os import PathLike

from . import mojang


class GameVersion:
    class Stable(GameVersion):
        def __new__(cls) -> Self: ...
    class Unstable(GameVersion):
        def __new__(cls) -> Self: ...
    class Name(GameVersion):
        def __new__(cls, name: str) -> Self: ...

class LoaderVersion:
    class Stable(LoaderVersion):
        def __new__(cls) -> Self: ...
    class Unstable(LoaderVersion):
        def __new__(cls) -> Self: ...
    class Name(LoaderVersion):
        def __new__(cls, name: str) -> Self: ...

class Installer(mojang.Installer):
    def __new__(cls, game_version: str | GameVersion | None = None, loader_version: str | LoaderVersion | None = None, main_dir: str | PathLike[str] | None = None) -> Self: ...
