"""Module providing tasks for launching forge mod loader versions.

The NeoForge support is still unstable API.
"""

from urllib import parse as url_parse
from zipfile import ZipFile
from pathlib import Path
from io import BytesIO
import subprocess
import shutil
import json
import os

from .standard import parse_download_entry, LIBRARIES_URL, \
    Context, VersionHandle, Version, Watcher, VersionNotFoundError

from .util import calc_input_sha1, LibrarySpecifier
from .http import http_request, HttpError

from typing import Dict, Optional, List, Tuple


_FORGE_REPO_URL = "https://maven.minecraftforge.net/"
_FORGE_GROUP = "net.minecraftforge"
_FORGE_ARTIFACT = "forge"

_NEO_FORGE_REPO_URL = "https://maven.neoforged.net/releases/"
_NEO_FORGE_GROUP = "net.neoforged"
_NEO_FORGE_ARTIFACT = "neoforge"


class ForgeVersion(Version):
    
    def __init__(self, forge_version: str = "release", *,
        context: Optional[Context] = None,
        prefix: str = "forge",
    ) -> None:

        super().__init__("", context=context)  # Do not give a root version for now.

        self.forge_version = forge_version
        self.prefix = prefix
        
        # This is set when version is resolved.
        self._forge_repo_url: Optional[str] = None
        self._forge_installer_spec: Optional[LibrarySpecifier] = None

        self._forge_post_info: Optional[_ForgePostInfo] = None
    
    def _resolve_version(self, watcher: Watcher) -> None:
        
        # Maybe "release" or "snapshot", we process this first.
        self.forge_version = self.manifest.filter_latest(self.forge_version)[0]

        # If no alias is specified, with add recommended.
        if "-" not in self.forge_version:
            self.forge_version = f"{self.forge_version}-recommended"

        # Now, if the specified version is an alias, we resolve it.
        if self.forge_version.endswith(("-latest", "-recommended")):

            # Split the version, used later.
            alias_version, alias = self.forge_version.rsplit("-", maxsplit=1)
            watcher.handle(ForgeResolveEvent(self.forge_version, True))

            # Try to get loader from promo versions.
            promo_versions = request_promo_versions()
            loader_version = promo_versions.get(self.forge_version)

            # If we can't find the load version, just try to other alias (issue #189).
            if loader_version is None:
                alias = { "latest": "recommended", "recommended": "latest" }[alias]
                self.forge_version = f"{alias_version}-{alias}"
                watcher.handle(ForgeResolveEvent(self.forge_version, True))
                loader_version = promo_versions.get(self.forge_version)

            if loader_version is None:
                raise VersionNotFoundError(f"{self.prefix}-{alias_version}-???")

            self.forge_version = f"{alias_version}-{loader_version}"
            watcher.handle(ForgeResolveEvent(self.forge_version, False))
        
        # Finally define the full version id.
        self.version = f"{self.prefix}-{self.forge_version}"
        self._forge_repo_url = _FORGE_REPO_URL
        self._forge_installer_spec = LibrarySpecifier(_FORGE_GROUP, _FORGE_ARTIFACT, self.forge_version, classifier="installer")

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        if version.id == self.version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:

        if version.id != self.version:
            return super()._fetch_version(version, watcher)
        
        # Must have been set in _resolve_version
        assert self._forge_repo_url is not None
        assert self._forge_installer_spec is not None

        # Extract the game version from the forge version, we'll use
        # it to add suffix to find the right forge version if needed.
        game_version = self.forge_version.split("-", 1)[0]

        # For some older game versions, some odd suffixes were used 
        # for the version scheme.
        suffixes = [""] + {
            "1.11":     ["-1.11.x"],
            "1.10.2":   ["-1.10.0"],
            "1.10":     ["-1.10.0"],
            "1.9.4":    ["-1.9.4"],
            "1.9":      ["-1.9.0", "-1.9"],
            "1.8.9":    ["-1.8.9"],
            "1.8.8":    ["-1.8.8"],
            "1.8":      ["-1.8"],
            "1.7.10":   ["-1.7.10", "-1710ls", "-new"],
            "1.7.2":    ["-mc172"],
        }.get(game_version, [])

        # Iterate suffix and find the first install JAR that works.
        install_jar = None
        original_version = self._forge_installer_spec.version
        for suffix in suffixes:
            try:
                # Apply the suffix before request...
                self._forge_installer_spec.version = f"{original_version}{suffix}"
                install_jar_url = f"{self._forge_repo_url}{self._forge_installer_spec.file_path()}"
                install_jar_res = res = http_request("GET", install_jar_url, accept="application/java-archive")
                install_jar = ZipFile(BytesIO(install_jar_res.data))
                break
            except HttpError as error:
                if error.res.status != 404:
                    raise
                # Silently ignore if the file was not found or forbidden.
                pass
        
        if install_jar is None:
            raise VersionNotFoundError(version.id)
        
        with install_jar:

            # The install profiles comes in multiples forms:
            # 
            # >= 1.12.2-14.23.5.2851
            #  There are two files, 'install_profile.json' which 
            #  contains processors and shared data, and `version.json`
            #  which is the raw version meta to be fetched.
            #
            # <= 1.12.2-14.23.5.2847
            #  There is only an 'install_profile.json' with the version
            #  meta stored in 'versionInfo' object. Each library have
            #  two keys 'serverreq' and 'clientreq' that should be
            #  removed when the profile is returned.

            try:
                info = install_jar.getinfo("install_profile.json")
                with install_jar.open(info) as fp:
                    install_profile = json.load(fp)
            except KeyError:
                raise ForgeInstallError(self.forge_version, ForgeInstallError.INSTALL_PROFILE_NOT_FOUND)

            # print(f"{json.dumps(install_profile, indent='  ')}")

            if "json" in install_profile:  # Forge versions since 1.12.2-14.23.5.2851
                
                info = install_jar.getinfo(install_profile["json"].lstrip("/"))
                with install_jar.open(info) as fp:
                    version.metadata = json.load(fp)

                # We use the bin directory if there is a need to extract temporary files.
                post_info = _ForgePostInfo(self.context.gen_bin_dir())

                # Parse processors
                for i, processor in enumerate(install_profile["processors"]):

                    processor_sides = processor.get("sides", [])
                    if not isinstance(processor_sides, list):
                        raise ValueError(f"forge profile: /json/processors/{i}/sides must be an array")

                    if len(processor_sides) and "client" not in processor_sides:
                        continue

                    processor_jar_name = processor.get("jar")
                    if not isinstance(processor_jar_name, str):
                        raise ValueError(f"forge profile: /json/processors/{i}/jar must be a string")

                    processor_spec = LibrarySpecifier.from_str(processor_jar_name)

                    post_info.processors.append(_ForgePostProcessor(
                        processor_spec,
                        [LibrarySpecifier.from_str(raw_spec) for raw_spec in processor.get("classpath", [])],
                        processor.get("args", []),
                        processor.get("outputs", {})
                    ))

                # Some early (still modern) installers (<= 1.16.5) embed the forge JAR,
                # we need to extract it given its path.
                forge_spec_raw = install_profile.get("path")
                if forge_spec_raw is not None:
                    lib_spec = LibrarySpecifier.from_str(forge_spec_raw)
                    lib_path = self.context.libraries_dir / lib_spec.file_path()
                    zip_extract_file(install_jar, f"maven/{lib_spec.file_path()}", lib_path)

                # We fetch all libraries used to build artifacts, and we store each path 
                # to each library here. These install profile libraries are only used for
                # building, and will be used in finalize task.
                for i, install_lib in enumerate(install_profile["libraries"]):

                    lib_name = install_lib["name"]
                    lib_spec = LibrarySpecifier.from_str(lib_name)
                    lib_artifact = install_lib["downloads"]["artifact"]
                    lib_path = self.context.libraries_dir / lib_spec.file_path()

                    # Ignore the library if it has already been specified, has been seen
                    # in neoforge installer...
                    if lib_spec in post_info.libraries:
                        continue
                    post_info.libraries[lib_spec] = lib_path
                    # print(lib_spec, lib_path)
                    
                    if len(lib_artifact["url"]):
                        self._dl.add(parse_download_entry(lib_artifact, lib_path, "forge profile: /json/libraries/"), verify=True)
                    else:
                        # The lib should be stored inside the JAR file, under maven/ directory.
                        zip_extract_file(install_jar, f"maven/{lib_spec.file_path()}", lib_path)

                # Just keep the 'client' values.
                install_data = install_profile["data"]
                if isinstance(install_data, dict):
                    for data_key, data_val in install_data.items():

                        data_val = str(data_val["client"])

                        # Refer to a file inside the JAR file.
                        if data_val.startswith("/"):
                            dst_path = post_info.tmp_dir / data_val[1:]
                            zip_extract_file(install_jar, data_val[1:], dst_path)
                            data_val = str(dst_path.absolute())  # Replace by the path of extracted file.

                        post_info.variables[data_key] = data_val
                
                self._forge_post_info = post_info

            else:  # Forge versions before 1.12.2-14.23.5.2847

                version.metadata = install_profile.get("versionInfo")
                if not isinstance(version.metadata, dict):
                    raise ForgeInstallError(self.forge_version, ForgeInstallError.VERSION_METADATA_NOT_FOUND)
                
                # Older versions have non standard keys for libraries.
                for version_lib in version.metadata["libraries"]:
                    if "serverreq" in version_lib:
                        del version_lib["serverreq"]
                    if "clientreq" in version_lib:
                        del version_lib["clientreq"]
                    if "checksums" in version_lib:
                        del version_lib["checksums"]
                    # Older versions used to require libraries that are no longer installed
                    # by parent versions, therefore it's required to add url if not 
                    # provided, pointing to maven central repository, for downloading.
                    if not version_lib.get("url"):
                        version_lib["url"] = LIBRARIES_URL
                
                # Old version (<= 1.6.4) of forge are broken, even on official launcher.
                # So we fix them by manually adding the correct inherited version.
                if "inheritsFrom" not in version.metadata:
                    version.metadata["inheritsFrom"] = install_profile["install"]["minecraft"]

                # For "old" installers, that have an "install" section.
                jar_entry_path = install_profile["install"]["filePath"]
                jar_spec = LibrarySpecifier.from_str(install_profile["install"]["path"])
                
                # Here we copy the forge jar stored to libraries.
                jar_path = self.context.libraries_dir / jar_spec.file_path()
                zip_extract_file(install_jar, jar_entry_path, jar_path)

        version.metadata["id"] = version.id
        version.write_metadata_file()
    
    def _resolve_jar(self, watcher: Watcher) -> None:
        super()._resolve_jar(watcher)
        self._finalize_forge(watcher)
    
    def _finalize_forge(self, watcher: Watcher) -> None:
        try:
            self._finalize_forge_internal(watcher)
        except:
            # We just intercept errors and remove the version metadata in case of errors,
            # this allows us to re-run the whole install on next attempt.
            try:
                self._hierarchy[0].metadata_file().unlink()
            except FileNotFoundError:
                pass  # Not a problem if the file isn't present.
            raise

    def _finalize_forge_internal(self, watcher: Watcher) -> None:
        """This step finalize the forge installation, after both JVM and version's JAR
        files has been resolved. This is not always used, it depends on installer's
        version.
        """

        info = self._forge_post_info
        if info is None:
            return
        
        assert self._jvm_path is not None, "_resolve_jvm(...) missing"
        assert self._jar_path is not None, "_resolve_jar(...) missing"
        
        # Download JVM files and version's JAR. This is used for finalization.
        self._download(watcher)

        # Additional missing variables, the version's jar file is the same as the vanilla
        # one, so we use its path.
        info.variables["SIDE"] = "client"
        info.variables["MINECRAFT_JAR"] = str(self._jar_path.absolute())

        def replace_install_args(txt: str) -> str:
            txt = txt.format_map(info.variables)
            # Replace the pattern [lib name] with lib path.
            if txt[0] == "[" and txt[-1] == "]":
                spec = LibrarySpecifier.from_str(txt[1:-1])
                txt = str((self.context.libraries_dir / spec.file_path()).absolute())
            elif txt[0] == "'" and txt[-1] == "'":
                txt = txt[1:-1]
            return txt

        for processor in info.processors:

            # Extract the main-class from manifest. Required because we cannot use 
            # both -cp and -jar.
            jar_path = info.libraries[processor.spec].absolute()
            main_class = None
            with ZipFile(jar_path) as jar_fp:
                with jar_fp.open("META-INF/MANIFEST.MF") as manifest_fp:
                    for manifest_line in manifest_fp.readlines():
                        if manifest_line.startswith(b"Main-Class: "):
                            main_class = manifest_line[12:].decode().strip()
                            break
            
            if main_class is None:
                raise ValueError(f"cannot find main class in {jar_path}")

            # Try to find the task name in the arguments, just for information purpose.
            if len(processor.args) >= 2 and processor.args[0] == "--task":
                task = processor.args[1].lower()
            else:
                task = {
                    "jarsplitter": "split_jar",
                    "ForgeAutoRenamingTool": "forge_auto_renaming",
                    "binarypatcher": "patch_binary",
                    "SpecialSource": "special_source_renaming",
                }.get(processor.spec.artifact, f"unknown({processor.spec})")

            # Compute the full arguments list.
            args = [
                str(self._jvm_path.absolute()),
                "-cp", os.pathsep.join([str(jar_path), *(str(info.libraries[lib_spec].absolute()) for lib_spec in processor.class_path)]),
                main_class,
                *(replace_install_args(arg) for arg in processor.args)
            ]

            watcher.handle(ForgePostProcessingEvent(task))

            completed = subprocess.run(args, cwd=self.context.work_dir, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            if completed.returncode != 0:
                raise ValueError("ERROR", completed.stdout)
            
            # If there are sha1, check them.
            for lib_name, expected_sha1 in processor.sha1.items():
                lib_name = replace_install_args(lib_name)
                expected_sha1 = replace_install_args(expected_sha1)
                with open(lib_name, "rb") as fp:
                    actual_sha1 = calc_input_sha1(fp)
                    if actual_sha1 != expected_sha1:
                        raise ValueError(f"invalid sha1 for '{lib_name}', got {actual_sha1}, expected {expected_sha1}")
        
        # Finally, remove the temporary directory.
        shutil.rmtree(info.tmp_dir, ignore_errors=True)

        watcher.handle(ForgePostProcessedEvent())


class _NeoForgeVersion(ForgeVersion):

    def __init__(self, neoforge_version: str = "release", *,
        context: Optional[Context] = None,
        prefix: str = "neoforge",
    ) -> None:
        super().__init__(neoforge_version, context=context, prefix=prefix)
    
    def _resolve_version(self, watcher: Watcher) -> None:
        
        # Maybe "release" or "snapshot", we process this first.
        self.forge_version = self.manifest.filter_latest(self.forge_version)[0]

        # The forge version is not fully specified.
        if "-" not in self.forge_version and self.forge_version.startswith("1."):

            watcher.handle(ForgeResolveEvent(self.forge_version, True, _api = "neoforge"))
            full_version = _request_neoforge_version(self.forge_version)

            if full_version is None:
                raise VersionNotFoundError(f"{self.prefix}-{self.forge_version}-???")
            
            self.forge_version = full_version
            watcher.handle(ForgeResolveEvent(self.forge_version, False, _api = "neoforge"))
        
        # Finally define the full version id.
        self.version = f"{self.prefix}-{self.forge_version}"
        
        # This is using the legacy forge artifact for 1.20.1 only.
        forge_artifact = _FORGE_ARTIFACT if self.forge_version.startswith("1.20.1-") else _NEO_FORGE_ARTIFACT

        self._forge_repo_url = _NEO_FORGE_REPO_URL
        self._forge_installer_spec = LibrarySpecifier(_NEO_FORGE_GROUP, forge_artifact, self.forge_version, classifier="installer")


class _ForgePostProcessor:
    """Describe the execution model of a post process.
    """
    __slots__ = "spec", "class_path", "args", "sha1"
    def __init__(self, spec: LibrarySpecifier, class_path: List[LibrarySpecifier], args: List[str], sha1: Dict[str, str]) -> None:
        self.spec = spec
        self.class_path = class_path
        self.args = args
        self.sha1 = sha1


class _ForgePostInfo:
    """Internal state, used only when forge installer is "modern" (>= 1.12.2-14.23.5.2851)
    describing data and post processors.
    """

    def __init__(self, tmp_dir: Path) -> None:
        self.tmp_dir = tmp_dir
        self.variables: Dict[str, str] = {}   # Data for variable replacements.
        self.libraries: Dict[LibrarySpecifier, Path] = {}  # Install-time libraries.
        self.processors: List[_ForgePostProcessor] = []


class ForgeInstallError(Exception):
    """Errors that can happen while trying to install forge.
    """

    INSTALL_PROFILE_NOT_FOUND = "install_profile_not_found"
    VERSION_METADATA_NOT_FOUND = "version_meta_not_found"

    def __init__(self, version: str, code: str):
        self.version = version
        self.code = code
    
    def __str__(self) -> str:
        return repr((self.version, self.code))


class ForgeResolveEvent:
    """Event triggered when the full forge version has been/is being resolved. 
    The 'alias' attribute specifies if an alias version is being resolved, if false the
    resolving has finished and we'll try to install the given version.
    """
    __slots__ = "forge_version", "alias", "_api"

    def __init__(self, forge_version: str, alias: bool, *, _api = "forge") -> None:
        self.forge_version = forge_version
        self.alias = alias
        self._api = _api

class ForgePostProcessingEvent:
    """Event triggered when a post processing task is starting.
    """
    __slots__ = "task",
    def __init__(self, task: str) -> None:
        self.task = task

class ForgePostProcessedEvent:
    """Event triggered when forge post processing has finished, the game is ready to run.
    """
    __slots__ = tuple()


def request_promo_versions() -> Dict[str, str]:
    """Request recommended and latest versions for each supported game release, this only
    works for forge version.
    """
    return http_request("GET", "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json", 
        accept="application/json").json()["promos"]


def _request_neoforge_version(game_version: str) -> Optional[str]:
    """Request a neoforge version from a game version. Because of the NeoForge API 
    returning any matching version number, this function will also check that the returned
    version is of the right game version.
    """

    game_version_parts = game_version.split(".")
    if len(game_version_parts) < 2 or len(game_version_parts) > 3:
        return None
    
    # If the "super-major" version is not "1", abort...
    if game_version_parts[0] != "1":
        return None
    
    # Special case for the first version NeoForged was introduced.
    if game_version_parts == ["1", "20", "1"]:
        url = "https://maven.neoforged.net/api/maven/latest/version/releases/net%2Fneoforged%2Fforge?filter=1.20.1-"
    else:
        # Just keep major and minor version number and construct the neoforge version prefix.
        filter_version = ".".join(game_version_parts[1:3])
        url = f"https://maven.neoforged.net/api/maven/latest/version/releases/net%2Fneoforged%2Fneoforge?filter={url_parse.quote(filter_version)}"

    try:
        ret = http_request("GET", url, accept="application/json").json()
    except HttpError as err:
        if err.res.status != 404:
            raise
        return None

    return ret.get("version")


def request_maven_versions() -> List[str]:
    """Internal function that parses maven metadata of forge in order to get all 
    supported forge versions.
    """

    text = http_request("GET", f"{_FORGE_REPO_URL}/net/minecraftforge/forge/maven-metadata.xml", 
        accept="application/xml").text()
    
    versions = list()
    last_idx = 0

    # It's not really correct to parse XML like this, but I find this
    # acceptable since the schema is well known and it should be a
    # little bit easier to do thing like this.
    while True:
        start_idx = text.find("<version>", last_idx)
        if start_idx == -1:
            break
        end_idx = text.find("</version>", start_idx + 9)
        if end_idx == -1:
            break
        versions.append(text[(start_idx + 9):end_idx])
        last_idx = end_idx + 10

    return versions


def request_install_jar(version: str) -> ZipFile:
    """deprecated"""
    res = http_request("GET", f"{_FORGE_REPO_URL}/net/minecraftforge/forge/{version}/forge-{version}-installer.jar",
        accept="application/java-archive")
    
    return ZipFile(BytesIO(res.data))


def zip_extract_file(zf: ZipFile, entry_path: str, dst_path: Path):
    """Special function used to extract a specific file entry to a destination. 
    This is different from ZipFile.extract because the latter keep the full entry's path.
    """
    dst_path.parent.mkdir(parents=True, exist_ok=True)
    with zf.open(entry_path) as src, dst_path.open("wb") as dst:
        shutil.copyfileobj(src, dst)

#
# Following classes are deprecated public class that should never have been private...
#

class ForgePostProcessor:
    """deprecated: This class should have been private...
    """
    __slots__ = "jar_name", "class_path", "args", "sha1"
    def __init__(self, jar_name: str, class_path: List[str], args: List[str], sha1: Dict[str, str]) -> None:
        self.jar_name = jar_name
        self.class_path = class_path
        self.args = args
        self.sha1 = sha1

class ForgePostInfo:
    """deprecated: This class should have been private..
    """

    def __init__(self, tmp_dir: Path) -> None:
        self.tmp_dir = tmp_dir
        self.variables: Dict[str, str] = {}   # Data for variable replacements.
        self.libraries: Dict[str, Path] = {}  # Install-time libraries.
        self.processors: List[ForgePostProcessor] = []
