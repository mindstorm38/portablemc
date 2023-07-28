"""Global utilities for the CLI.
"""

from datetime import datetime

from portablemc.util import LibrarySpecifier, from_iso_date

from typing import Optional, Union


def format_locale_date(raw: Union[str, float]) -> str:
    if isinstance(raw, float):
        return datetime.fromtimestamp(raw).strftime("%c")
    else:
        return from_iso_date(str(raw)).strftime("%c")


def format_time(timestamp: float) -> str:
    """Format the given timestamp (in seconds) into hh:mm:ss format.
    """
    return datetime.fromtimestamp(timestamp).strftime("%H:%M:%S")


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
    

def anonymize_email(email: str) -> str:
    """Return a visually anonymized email.
    """

    def anonymize_part(email_part: str) -> str:
        return f"{email_part[0]}{'*' * (len(email_part) - 2)}{email_part[-1]}"
    
    parts = []

    for i, part in enumerate(email.split("@", maxsplit=1)):
        if i == 0:
            parts.append(anonymize_part(part))
        else:
            parts.append(".".join((anonymize_part(server_part) if j == 0 else server_part for j, server_part in enumerate(part.split(".", maxsplit=1)))))
    
    return "@".join(parts)


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

        return cls(parts[0], 
                   parts[1] or None if len(parts) >= 2 else None, 
                   parts[2] or None if len(parts) >= 3 else None)

    def matches(self, spec: LibrarySpecifier) -> bool:
        return self.artifact == spec.artifact \
            and (self.version is None or self.version == spec.version) \
            and (self.classifier is None or (spec.classifier or "").startswith(self.classifier))

    def __str__(self) -> str:
        return f"{self.artifact}:{self.version or ''}" + ("" if self.classifier is None else f":{self.classifier}")

    def __repr__(self) -> str:
        return f"<LibrarySpecifierFilter {self}>"