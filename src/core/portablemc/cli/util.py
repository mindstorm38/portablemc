"""Global utilities for the CLI.
"""

from datetime import datetime

from ..util import LibrarySpecifier, from_iso_date

from typing import Optional, Union


def format_locale_date(raw: Union[str, float]) -> str:
    if isinstance(raw, float):
        return datetime.fromtimestamp(raw).strftime("%c")
    else:
        return from_iso_date(str(raw)).strftime("%c")


class LibrarySpecifierFilter:
    """A filter for library specifier, used with the start command to exclude some 
    libraries.
    """
    
    __slots__ = "artifact", "version", "classifier"

    def __init__(self, artifact: str, version: Optional[str], classifier: Optional[str]):
        self.artifact = artifact
        self.version = version
        self.classifier = classifier
    
    @classmethod
    def from_str(cls, s: str) -> "LibrarySpecifierFilter":

        parts = s.split(":")
        if len(parts) > 3:
            raise ValueError("Invalid parts count")

        return cls(*map(lambda part: part or None, parts))

    def matches(self, spec: LibrarySpecifier) -> bool:
        return self.artifact == spec.artifact \
            and (self.version is None or self.version == spec.version) \
            and (self.classifier is None or (spec.classifier or "").startswith(self.classifier))

    def __str__(self) -> str:
        return f"{self.artifact}:{self.version or ''}" + ("" if self.classifier is None else f":{self.classifier}")
