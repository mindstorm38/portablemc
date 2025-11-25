from typing import Self
from os import PathLike


class Version:
    class Stable(Version):
        def __new__(cls, game_version: str) -> Self: ...
    class Unstable(Version):
        def __new__(cls, game_version: str) -> Self: ...
    class Name(Version):
        def __new__(cls, name: str) -> Self: ...

class Installer:
    def __new__(cls, version: str | Version | None = None, main_dir: str | PathLike[str] | None = None) -> Self: ...
