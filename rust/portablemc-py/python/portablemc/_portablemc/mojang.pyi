from typing import Self
from os import PathLike

from .standard import Installer as StandardInstaller

class Version:
    class Release:
        def __new__(cls) -> Self: ...
    class Snapshot:
        def __new__(cls) -> Self: ...
    class Name:
        def __new__(cls, name: str) -> Self: ...

class Installer(StandardInstaller):
    def __new__(cls, version: str | Version | None = None, main_dir: str | PathLike[str] | None = None) -> Self: ...
