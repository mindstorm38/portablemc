from typing import Self
from enum import Enum, auto

from . import mojang, standard


class Loader(Enum):
    Forge = auto()
    NeoForge = auto()


class Version:
    class Stable(Version):
        def __new__(cls, game_version: str) -> Self: ...
    class Unstable(Version):
        def __new__(cls, game_version: str) -> Self: ...
    class Name(Version):
        def __new__(cls, name: str) -> Self: ...


class Installer(mojang.Installer):

    def __new__(cls, loader: Loader, version: Version) -> Self: ...

    def __repr__(self) -> str: ...

    @property
    def loader(self) -> Loader: ...
    @loader.setter
    def loader(self, loader: Loader): ...
    
    @mojang.Installer.version.getter
    def version(self) -> Version: ...
    @version.setter
    def version(self, version: Version): ...

    def install(self) -> standard.Game: ...
