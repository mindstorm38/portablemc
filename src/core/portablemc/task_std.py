"""Definition of the standard tasks for vanilla Minecraft, versions
are provided by Mojang through their version manifest (see associated
module).
"""

from json import JSONDecodeError
from pathlib import Path
import hashlib
import json

from .task import Task, TaskError
from .manifest import VersionManifest

from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from typing import Tuple, Optional


class VersionMetadataTask(Task):
    """Version metadata resolving.

    This task resolves the current version's metadata and inherited 
    versions.

    :input versions_dir: The path to the versions directory.
    :input version_id: The version identifier to resolve and loads
    metadata from.
    :input version_manifest: Optional, version manifest used to
    download official versions.
    """

    ERROR_TOO_MUCH_PARENTS = "too_much_parents"
    ERROR_NOT_FOUND = "not_found"

    def __init__(self, max_parents: int = 10) -> None:
        self.max_parents: int = max_parents
    
    def execute(self, state: dict) -> None:

        versions_dir: Path = state["versions_dir"]
        version_id: str = state["version_id"]
        version_manifest: VersionManifest = state.get("version_manifest") or VersionManifest()

        max_parents = self.max_parents

        version_meta, version_dir = self.ensure_version_meta(versions_dir, version_id)
        while "inheritsFrom" in version_meta:
            if max_parents <= 0:
                raise TaskError(self.ERROR_TOO_MUCH_PARENTS)
            max_parents -= 1
            parent_meta, _ = self.ensure_version_meta(versions_dir, version_meta["inheritsFrom"])
            del version_meta["inheritsFrom"]
            merge_dict(version_meta, parent_meta)
        
        state["version_meta"] = version_meta
        state["version_dir"] = version_dir
    
    def ensure_version_meta(self, 
        versions_dir: "Path", 
        version_id: str
    ) -> "Tuple[dict, Path]":
        """This function tries to load and get the directory path of
        a given version id. This function proceeds in multiple steps:
        it tries to load the version's metadata, if the metadata is
        found then it's validated with the `validate_version_meta`
        method. If the version was not found or is not valid, then
        the metadata is fetched by `fetch_version_meta` method.

        :param versions_dir: Path to 
        :param version_id: _description_
        :raises TaskError: _description_
        :return: _description_
        """

        version_dir = versions_dir / version_id
        version_meta_file = version_dir / f"{version_id}.json"

        try:
            with version_meta_file.open("rt") as version_meta_fp:
                version_meta = json.load(version_meta_fp)
        except (OSError, JSONDecodeError):
            version_meta = None

        if version_meta is not None:
            if self.validate_version_meta(version_id, version_dir, version_meta_file, version_meta):
                return version_meta, version_dir
            else:
                version_meta = None

        if version_meta is None:
            try:
                version_meta = self.fetch_version_meta(version_id, version_dir, version_meta_file)
                if "_pmc_no_dump" not in version_meta:
                    version_dir.mkdir(parents=True, exist_ok=True)
                    with version_meta_file.open("wt") as version_meta_fp:
                        json.dump(version_meta, version_meta_fp, indent=2)
            except NotADirectoryError:
                raise TaskError(self.ERROR_NOT_FOUND)

        return version_meta, version_dir

    def validate_version_meta(self, 
        version_id: str, 
        version_dir: "Path", 
        version_meta_file: "Path", 
        version_meta: dict
    ) -> bool:
        """This function checks that version metadata is correct.

        An internal method to check if a version's metadata is 
        up-to-date, returns `True` if it is. If `False`, the version 
        metadata is re-fetched, this is the default when version 
        metadata doesn't exist.

        The default implementation check official versions against the
        expected SHA1 hash of the metadata file.

        Args:
            version_id (str): _description_
            version_dir (Path): _description_
            version_meta_file (Path): _description_
            version_meta (dict): _description_

        Returns:
            bool: True if the given version 
        """

        version_super_meta = self._ensure_version_manifest().get_version(version_id)
        if version_super_meta is None:
            return True
        else:
            expected_sha1 = version_super_meta.get("sha1")
            if expected_sha1 is not None:
                try:
                    with version_meta_file.open("rb") as version_meta_fp:
                        current_sha1 = calc_input_sha1(version_meta_fp)
                        return expected_sha1 == current_sha1
                except OSError:
                    return False
            return True

    def fetch_version_meta(self, 
        version_id: str,
        version_dir: "Path",
        version_meta_file: "Path"
    ) -> dict:

        """
        An internal method to fetch a version metadata. The returned dict can contain an optional
        key '_pmc_no_dump' that prevent dumping the dictionary as JSON to the metadata file.

        The default implementation fetch from official versions, and directly write the read data
        to the meta file in order to keep the exact same SHA1 hash of the metadata. The default
        implementation also set the `_pmc_no_dump` flag to the returned data in order to avoid
        overwriting the file.
        """

        version_super_meta = self._ensure_version_manifest().get_version(version_id)
        if version_super_meta is None:
            raise VersionError(VersionError.NOT_FOUND, version_id)
        code, raw_data = http_request(version_super_meta["url"], "GET")
        if code != 200:
            raise VersionError(VersionError.NOT_FOUND, version_id)
        version_dir.mkdir(parents=True, exist_ok=True)
        with version_meta_file.open("wb") as fp:
            fp.write(raw_data)
        try:
            data = json.loads(raw_data)
            data["_pmc_no_dump"] = True
            return data
        except JSONDecodeError:
            raise VersionError(VersionError.NOT_FOUND, version_id)


def merge_dict(dst: dict, other: dict) -> None:
    """Merge a dictionary into a destination one.

    Merge the `other` dict into the `dst` dict. For every key/value in
    `other`, if the key is present in `dst`it does nothing. Unless 
    values in both dict are also dict, in this case the merge is 
    recursive. If the value in both dict are list, the 'dst' list is 
    extended (.extend()) with the one of `other`.

    :param dst: The source dictionary to merge `other` into.
    :param other: The dictionary merged into `dst`.
    """

    for k, v in other.items():
        if k in dst:
            if isinstance(dst[k], dict) and isinstance(v, dict):
                merge_dict(dst[k], v)
            elif isinstance(dst[k], list) and isinstance(v, list):
                dst[k].extend(v)
        else:
            dst[k] = v


def calc_input_sha1(input_stream, *, buffer_len: int = 8192) -> str:
    """Internal function to calculate the sha1 of an input stream.

    :param input_stream: The input stream that supports `readinto`.
    :param buffer_len: Internal buffer length, defaults to 8192
    :return: The sha1 string.
    """
    h = hashlib.sha1()
    b = bytearray(buffer_len)
    mv = memoryview(b)
    for n in iter(lambda: input_stream.readinto(mv), 0):
        h.update(mv[:n])
    return h.hexdigest()
