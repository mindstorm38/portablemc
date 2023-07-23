"""Global utilities used internally. The functions can be used externally but upward
compatibility is not guaranteed unless explicitly specified.
"""

from datetime import datetime
import platform

from typing import Optional


jvm_bin_filename = "javaw.exe" if platform.system() == "Windows" else "java"


def merge_dict(dst: dict, other: dict) -> None:
    """Merge a dictionary into a destination one.

    Merge the `other` dict into the `dst` dict. For every key/value in `other`, if the key
    is present in `dst`it does nothing. Unless values in both dict are also dict, in this
    case the merge is recursive. If the value in both dict are list, the 'dst' list is 
    extended (.extend()) with the one of `other`. If a key is present in both `dst` and
    `other` but with different types, the value is not overwritten.

    :param dst: The source dictionary to merge `other` into.
    :param other: The dictionary merged into `dst`.
    """

    for k, v in other.items():
        if k in dst:
            dst_v = dst[k]
            if isinstance(dst_v, dict) and isinstance(v, dict):
                merge_dict(dst_v, v)
            elif isinstance(dst_v, list) and isinstance(v, list):
                dst[k] = v + dst_v
        else:
            dst[k] = v


def calc_input_sha1(input_stream, *, buffer_len: int = 8192) -> str:
    """Internal function to calculate the sha1 of an input stream.

    :param input_stream: The input stream that supports `readinto`.
    :param buffer_len: Internal buffer length, defaults to 8192
    :return: The sha1 string.
    """
    import hashlib
    h = hashlib.sha1()
    b = bytearray(buffer_len)
    mv = memoryview(b)
    for n in iter(lambda: input_stream.readinto(mv), 0):
        h.update(mv[:n])
    return h.hexdigest()


def from_iso_date(raw: str) -> datetime:
    """Replacement for `datetime.fromisoformat()` which is missing from Python 3.6. This 
    function replace it if needed.

    Currently, only a subset of the ISO format is supported, both hours, minutes and 
    seconds must be defined and the timezone, if present must contain both hours and 
    minutes, no more.
    """
    if hasattr(datetime, "fromisoformat"):
        return datetime.fromisoformat(raw)
    from datetime import timezone, timedelta
    tz_idx = raw.find("+")
    dt = datetime.strptime(raw[:tz_idx], "%Y-%m-%dT%H:%M:%S")
    if tz_idx != -1:
        tz_dt = datetime.strptime(raw[tz_idx + 1:], "%H:%M")
        dt = dt.replace(tzinfo=timezone(timedelta(hours=tz_dt.hour, minutes=tz_dt.minute)))
    return dt


class LibrarySpecifier:
    """A maven-style library specifier.
    """

    __slots__ = "group", "artifact", "version", "classifier", "extension"

    def __init__(self, group: str, artifact: str, version: str, classifier: Optional[str] = None, extension: str = "jar"):
        self.group = group
        self.artifact = artifact
        self.version = version
        self.classifier = classifier
        self.extension = extension
    
    @classmethod
    def from_str(cls, s: str) -> "LibrarySpecifier":
        """Parse a library specifier string 'group:artifact:version[:classifier]'.
        """

        ext_split = s.rsplit("@", maxsplit=1)
        ext = "jar" if len(ext_split) == 1 else ext_split[1]

        if not len(ext):
            raise ValueError("invalid library specifier: empty extension")
        
        parts = ext_split[0].split(":", 3)

        if len(parts) < 3:
            raise ValueError("invalid library specifier: too few parts")
        else:
            return LibrarySpecifier(parts[0], parts[1], parts[2], parts[3] if len(parts) == 4 else None, ext)

    def __str__(self) -> str:
        return f"{self.group}:{self.artifact}:{self.version}" + \
            ("" if self.classifier is None else f":{self.classifier}") + \
            ("" if self.extension == "jar" else f"@{self.extension}")

    def __eq__(self, other) -> bool:
        return isinstance(other, LibrarySpecifier) and \
            (self.group, self.artifact, self.version, self.classifier, self.extension) == \
            (other.group, other.artifact, other.version, other.classifier, other.extension)

    def __repr__(self) -> str:
        return f"<LibrarySpecifier {self}>"
    
    def __hash__(self) -> int:
        return hash((self.group, self.artifact, self.version, self.classifier, self.extension))

    def file_path(self) -> str:
        """Return the standard path to store the file of this specifier.
        
        The path separator will always be forward slashes '/', because it's compatible 
        with linux/mac/windows and URL paths.

        Specifier `com.foo.bar:artifact:version@zip` gives 
        `com/foo/bar/artifact/version/artifact-version.zip`.
        """
        
        file_name = f"{self.artifact}-{self.version}" + \
            ("" if self.classifier is None else f"-{self.classifier}") + \
            f".{self.extension}"
        
        return "/".join([*self.group.split("."), self.artifact, self.version, file_name])
