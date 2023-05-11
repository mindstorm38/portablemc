"""Definition of the standard tasks for vanilla Minecraft, versions
are provided by Mojang through their version manifest (see associated
module).
"""

from json import JSONDecodeError
from pathlib import Path
import platform
import hashlib
import json

from .manifest import VersionManifest, VersionManifestError
from .task import Task, TaskError, State
from .http import http_request

from typing import Optional


class Context:
    """Base class for storing global context of a standard Minecraft launch, such as main 
    and working directory.
    """

    def __init__(self, 
        main_dir: Optional[Path] = None, 
        work_dir: Optional[Path] = None
    ) -> None:
        """Construct a Minecraft installation context. This context is used by most of the
        installer's tasks to know where to install files and from where to launch the game.

        :param main_dir: The main directory where versions, assets, libraries and 
        optionally JVM are installed. If not specified this path will be set the usual 
        `.minecraft` (see https://minecraft.fandom.com/fr/wiki/.minecraft).
        :param work_dir: The working directory from where the game is run, the game stores
        thing like saves, resource packs, options and mods if relevant. This defaults to
        `main_dir` if not specified.
        """

        main_dir = get_minecraft_dir() if main_dir is None else main_dir
        self.work_dir = main_dir if work_dir is None else work_dir
        self.versions_dir = main_dir / "versions"
        self.assets_dir = main_dir / "assets"
        self.libraries_dir = main_dir / "libraries"
        self.jvm_dir = main_dir / "jvm"
        self.bin_dir = self.work_dir / "bin"


class VersionId:
    """Small class used to specify which Minecraft version to launch.
    """

    __slots__ = "id",

    def __init__(self, id: str) -> None:
        self.id = id


class VersionMetadata:
    """This class holds version's metadata, as resolved by `MetadataTask`.
    """

    __slots__ = "id", "dir", "data",

    def __init__(self, id: str, dir: Path) -> None:
        self.id = id
        self.dir = dir
        self.data = {}
    
    def metadata_file(self) -> Path:
        """This function returns the computed path of the metadata file.
        """
        return self.dir / f"{self.id}.json"

    def jar_file(self) -> Path:
        """This function returns the computed path of the JAR file of the game.
        """
        return self.dir / f"{self.id}.jar"

    def write_metadata_file(self) -> None:
        """This function write the metadata file of the version with the internal data.
        """
        self.dir.mkdir(parents=True, exist_ok=True)
        with self.metadata_file().open("wt") as fp:
            json.dump(self.data, fp)

    def read_metadata_file(self) -> bool:
        """This function reads the metadata file and updates the internal data if found.

        :return: True if the data was actually updated from the file.
        """
        try:
            with self.metadata_file().open("rt") as fp:
                self.data = json.load(fp)
            return True
        except (OSError, JSONDecodeError):
            return False


class MetadataTask(Task):
    """Version metadata resolving.

    This task resolves the current version's metadata and inherited 
    versions.

    :in Context: The installation context.
    :in VersionId: Describe which version to resolve.
    :out VersionMetadata: The resolved version metadata.
    """

    ERROR_TOO_MUCH_PARENTS = "too_much_parents"
    ERROR_NOT_IN_MANIFEST = "not_in_manifest"
    ERROR_HTTP = "http"
    ERROR_JSON = "json"

    def __init__(self, *, 
        max_parents: int = 10, 
        manifest: Optional[VersionManifest] = None
    ) -> None:
        self.max_parents = max_parents
        self.manifest = manifest

    def execute(self, state: State) -> None:
        
        max_parents = self.max_parents

        version = state[VersionId]
        context = state[Context]

        version_id: Optional[str] = version.id
        version_meta: Optional[VersionMetadata] = None

        while True:

            version_meta_parent = VersionMetadata(version_id, context.versions_dir / version_id)
            self.ensure_version_meta(version_meta_parent)

            if version_meta is None:
                version_meta = version_meta_parent
            else:
                merge_dict(version_meta, version_meta_parent.data)

            version_id = version_meta.data.pop("inheritsFrom", None)
            if version_id is None:
                break
        
        # The metadata is included to the state.
        state.insert(version_meta)
    
    def ensure_manifest(self) -> VersionManifest:
        """ Ensure that a version manifest exists for fetching official versions.
        """
        if self.manifest is None:
            self.manifest = VersionManifest()
        return self.manifest

    def ensure_version_meta(self, version_metadata: VersionMetadata) -> None:
        """This function tries to load and get the directory path of a given version id.
        This function proceeds in multiple steps: it tries to load the version's metadata,
        if the metadata is found then it's validated with the `validate_version_meta`
        method. If the version was not found or is not valid, then the metadata is fetched
        by `fetch_version_meta` method.

        :param version_metadata: The version metadata to fill with 
        """

        if version_metadata.read_metadata_file():
            if self.validate_version_meta(version_metadata):
                # If the version is successfully loaded and valid, return as-is.
                return
        
        # If not loadable or not validated, fetch metadata.
        self.fetch_version_meta(version_metadata)

    def validate_version_meta(self, version_meta: VersionMetadata) -> bool:
        """This function checks that version metadata is correct.

        An internal method to check if a version's metadata is 
        up-to-date, returns `True` if it is. If `False`, the version 
        metadata is re-fetched, this is the default when version 
        metadata doesn't exist.

        The default implementation check official versions against the
        expected SHA1 hash of the metadata file.

        :param version_meta: The version metadata to validate or not.
        :return: True if the given version.
        """

        version_super_meta = self.ensure_manifest().get_version(version_meta.id)
        if version_super_meta is None:
            return True
        else:
            expected_sha1 = version_super_meta.get("sha1")
            if expected_sha1 is not None:
                try:
                    with version_meta.metadata_file().open("rb") as version_meta_fp:
                        current_sha1 = calc_input_sha1(version_meta_fp)
                        return expected_sha1 == current_sha1
                except OSError:
                    return False
            return True

    def fetch_version_meta(self, version_meta: VersionMetadata) -> None:
        """Internal method to fetch the data of the given version.

        :param version_meta: The version meta to fetch data into.
        :raises VersionError: _description_
        :raises VersionError: _description_
        :raises VersionError: _description_
        """

        version_super_meta = self.ensure_manifest().get_version(version_meta.id)
        if version_super_meta is None:
            raise TaskError(self.ERROR_NOT_IN_MANIFEST)
        
        code, raw_data = http_request(version_super_meta["url"], "GET")
        if code != 200:
            raise TaskError(self.ERROR_HTTP)
        
        # First decode the data and set it to the version meta.
        try:
            version_meta.data = json.loads(raw_data)
        except JSONDecodeError:
            raise TaskError(self.ERROR_JSON)
        
        # If successful, write the raw data directly to the file.
        version_meta.dir.mkdir(parents=True, exist_ok=True)
        with version_meta.metadata_file().open("wb") as fp:
            fp.write(raw_data)
 

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


def get_minecraft_dir() -> "Path":
    """Internal function to get the default directory for installing
    and running Minecraft.
    """
    home = Path.home()
    return {
        "Windows": home.joinpath("AppData", "Roaming", ".minecraft"),
        "Darwin": home.joinpath("Library", "Application Support", "minecraft"),
    }.get(platform.system(), home / ".minecraft")
