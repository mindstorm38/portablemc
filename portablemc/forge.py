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

from typing import Dict, Optional, List


_FORGE_REPO = "https://maven.minecraftforge.net/net/minecraftforge/forge"
_NEO_FORGE_REPO = "https://maven.neoforged.net/releases/net/neoforged/forge"


class ForgeVersion(Version):
    
    def __init__(self, forge_version: str = "release", *,
        context: Optional[Context] = None,
        prefix: str = "forge",
        _forge_repo: str = _FORGE_REPO,
    ) -> None:

        super().__init__("", context=context)  # Do not give a root version for now.

        self.forge_version = forge_version
        self.prefix = prefix
        self._forge_repo = _forge_repo
        self._forge_post_info: Optional[ForgePostInfo] = None
    
    def _resolve_version(self, watcher: Watcher) -> None:
        
        # Maybe "release" or "snapshot", we process this first.
        self.forge_version = self.manifest.filter_latest(self.forge_version)[0]

        if self._forge_repo == _FORGE_REPO:

            # No dash or alias version, resolve against promo version.
            alias = self.forge_version.endswith(("-latest", "-recommended"))
            if "-" not in self.forge_version or alias:

                # If it's not an alias, create the alias from the game version.
                alias_version = self.forge_version if alias else f"{self.forge_version}-recommended"
                watcher.handle(ForgeResolveEvent(alias_version, True, _forge_repo=_FORGE_REPO))

                # Try to get loader from promo versions.
                promo_versions = request_promo_versions()
                loader_version = promo_versions.get(alias_version)

                # Try with "-latest", some version do not have recommended.
                if loader_version is None and not alias:
                    alias_version = f"{self.forge_version}-latest"
                    watcher.handle(ForgeResolveEvent(alias_version, True, _forge_repo=_FORGE_REPO))
                    loader_version = promo_versions.get(alias_version)
                
                # Remove alias
                last_dash = alias_version.rindex("-")
                alias_version = alias_version[:last_dash]

                if loader_version is None:
                    raise VersionNotFoundError(f"{self.prefix}-{alias_version}-???")

                self.forge_version = f"{alias_version}-{loader_version}"

                watcher.handle(ForgeResolveEvent(self.forge_version, False, _forge_repo=_FORGE_REPO))
        
        elif self._forge_repo == _NEO_FORGE_REPO:

            # The forge version is not fully specified.
            if "-" not in self.forge_version:

                watcher.handle(ForgeResolveEvent(self.forge_version, True, _forge_repo=_NEO_FORGE_REPO))
                full_version = _request_neoforge_version(self.forge_version)

                if full_version is None:
                    raise VersionNotFoundError(f"{self.prefix}-{self.forge_version}-???")
                
                self.forge_version = full_version
                watcher.handle(ForgeResolveEvent(self.forge_version, False, _forge_repo=_NEO_FORGE_REPO))
        
        # Finally define the full version id.
        self.version = f"{self.prefix}-{self.forge_version}"

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        if version.id == self.version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:

        if version.id != self.version:
            return super()._fetch_version(version, watcher)
        
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
        for suffix in suffixes:
            try:
                install_jar = request_install_jar(f"{self.forge_version}{suffix}", _repo=self._forge_repo)
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

            # print(f"{install_profile=}")

            if "json" in install_profile:

                # Forge versions since 1.12.2-14.23.5.2851
                info = install_jar.getinfo(install_profile["json"].lstrip("/"))
                with install_jar.open(info) as fp:
                    version.metadata = json.load(fp)

                # We use the bin directory if there is a need to extract temporary files.
                post_info = ForgePostInfo(self.context.gen_bin_dir())

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

                    post_info.processors.append(ForgePostProcessor(
                        processor_jar_name,
                        processor.get("classpath", []),
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

                    post_info.libraries[lib_name] = lib_path
                    
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

            else: 

                # Forge versions before 1.12.2-14.23.5.2847
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
                    # Older version uses to require libraries that are no longer installed
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
            jar_path = info.libraries[processor.jar_name].absolute()
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
            elif processor.jar_name.startswith("net.minecraftforge:jarsplitter:"):
                task = "split_jar"
            elif processor.jar_name.startswith("net.minecraftforge:ForgeAutoRenamingTool:"):
                task = "forge_auto_renaming"
            elif processor.jar_name.startswith("net.minecraftforge:binarypatcher:"):
                task = "patch_binary"
            elif processor.jar_name.startswith("net.md-5:SpecialSource:"):
                task = "special_source_renaming"
            else:
                task = "unknown"

            # Compute the full arguments list.
            args = [
                str(self._jvm_path.absolute()),
                "-cp", os.pathsep.join([str(jar_path), *(str(info.libraries[lib_name].absolute()) for lib_name in processor.class_path)]),
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


class ForgePostProcessor:
    """Describe the execution model of a post process.
    """
    __slots__ = "jar_name", "class_path", "args", "sha1"
    def __init__(self, jar_name: str, class_path: List[str], args: List[str], sha1: Dict[str, str]) -> None:
        self.jar_name = jar_name
        self.class_path = class_path
        self.args = args
        self.sha1 = sha1


class ForgePostInfo:
    """Internal state, used only when forge installer is "modern" (>= 1.12.2-14.23.5.2851)
    describing data and post processors.
    """

    def __init__(self, tmp_dir: Path) -> None:
        self.tmp_dir = tmp_dir
        self.variables: Dict[str, str] = {}   # Data for variable replacements.
        self.libraries: Dict[str, Path] = {}  # Install-time libraries.  FIXME: Get rid of this?
        self.processors: List[ForgePostProcessor] = []


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
    __slots__ = "forge_version", "alias", "_forge_repo"
    def __init__(self, forge_version: str, alias: bool, *, _forge_repo: str) -> None:
        self.forge_version = forge_version
        self.alias = alias
        self._forge_repo = _forge_repo

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
    works.
    """
    return http_request("GET", "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json", 
        accept="application/json").json()["promos"]


def _request_neoforge_version(game_version: str) -> Optional[str]:
    """Request a neoforge version from a game version. Because of the NeoForge API 
    returning any matching version number, this function will also check that the returned
    version is of the right game version.
    """
    try:
        # NOTE: For now we don't sanitize the parameter.
        url = f"https://maven.neoforged.net/api/maven/latest/version/releases/net%2Fneoforged%2Fforge?filter={url_parse.quote(game_version)}"
        ret = http_request("GET", url, accept="application/json").json()
        loader_version = ret.get("version", "")
        if not loader_version.startswith(f"{game_version}-"):
            return None
        return loader_version
    except HttpError as err:
        if err.res.status != 404:
            raise
        return None


def request_maven_versions(*, _repo: str = _FORGE_REPO) -> List[str]:
    """Internal function that parses maven metadata of forge in order to get all 
    supported forge versions.
    """

    text = http_request("GET", f"{_repo}/maven-metadata.xml", 
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


def request_install_jar(version: str, *, _repo: str = _FORGE_REPO) -> ZipFile:
    """Internal function to request the installation JAR file.
    """
    res = http_request("GET", f"{_repo}/{version}/forge-{version}-installer.jar",
        accept="application/java-archive")
    
    return ZipFile(BytesIO(res.data))


def zip_extract_file(zf: ZipFile, entry_path: str, dst_path: Path):
    """Special function used to extract a specific file entry to a destination. 
    This is different from ZipFile.extract because the latter keep the full entry's path.
    """
    dst_path.parent.mkdir(parents=True, exist_ok=True)
    with zf.open(entry_path) as src, dst_path.open("wb") as dst:
        shutil.copyfileobj(src, dst)
