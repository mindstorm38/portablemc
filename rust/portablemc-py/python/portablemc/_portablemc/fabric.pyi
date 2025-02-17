from typing import Self
from enum import Enum, auto

from . import mojang, standard


class Loader(Enum):
    Fabric = auto()
    Quilt = auto()
    LegacyFabric = auto()
    Babric = auto()


class GameVersion(Enum):
    Stable = auto()
    Unstable = auto()


class LoaderVersion(Enum):
    Stable = auto()
    Unstable = auto()


class Installer(mojang.Installer):

    def __new__(cls, loader: Loader, game_version: str | GameVersion = GameVersion.Stable, loader_version: str | LoaderVersion = LoaderVersion.Stable) -> Self: ...
    
    @property
    def loader(self) -> Loader: ...
    @loader.setter
    def loader(self, loader: Loader): ...
    
    @property
    def game_version(self) -> str | GameVersion: ...
    @game_version.setter
    def game_version(self, game_version: str | GameVersion): ...
    
    @property
    def loader_version(self) -> str | LoaderVersion: ...
    @loader_version.setter
    def loader_version(self, loader_version: str | LoaderVersion): ...

    def install(self) -> standard.Game: ...
