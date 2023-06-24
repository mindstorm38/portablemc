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


def format_number(n: float) -> str:
    """Return a number with suffix k, M, G or nothing. 
    The string is at most 7 chars unless the size exceed 1 T.
    """
    if n < 1000:
        return f"{int(n)}"
    elif n < 1000000:
        return f"{(int(n / 100) / 10):.1f} k"
    elif n < 1000000000:
        return f"{(int(n / 100000) / 10):.1f} M"
    else:
        return f"{(int(n / 100000000) / 10):.1f} G"


def format_duration(n: float) -> str:
    """Return a duration with proper suffix s, m, h.
    """
    if n < 60:
        return f"{int(n)} s"
    elif n < 3600:
        return f"{int(n / 60)} m"
    else:
        return f"{int(n / 3600)} h"


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

        return cls(parts[0], parts[1] or None, parts[2] or None)

    def matches(self, spec: LibrarySpecifier) -> bool:
        return self.artifact == spec.artifact \
            and (self.version is None or self.version == spec.version) \
            and (self.classifier is None or (spec.classifier or "").startswith(self.classifier))

    def __str__(self) -> str:
        return f"{self.artifact}:{self.version or ''}" + ("" if self.classifier is None else f":{self.classifier}")
