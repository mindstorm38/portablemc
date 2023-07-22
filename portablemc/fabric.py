"""Definition of tasks for installing and running Fabric/Quilt mod loader.
"""

from .vanilla import MetadataRoot, MetadataTask, VersionRepository, Version, \
    VersionRepositories, VersionNotFoundError, Context, VersionManifest
from .task import Task, State, Watcher, Sequence
from .http import http_request, HttpError

from typing import Optional, Any, Iterator


class FabricApi:
    """This class is internally used to defined two constant for both official Fabric
    backend API and Quilt API which have the same endpoints. So we use the same logic
    for both mod loaders.
    """

    def __init__(self, name: str, api_url: str) -> None:
        self.name = name
        self.api_url = api_url
    
    def request_fabric_meta(self, method: str) -> Any:
        """Generic HTTP request to the fabric's REST API.
        """
        return http_request("GET", f"{self.api_url}{method}", accept="application/json").json()

    def request_fabric_loader_version(self, vanilla_version: str) -> Optional[str]:
        loaders = self.request_fabric_meta(f"versions/loader/{vanilla_version}")
        return loaders[0].get("loader", {}).get("version") if len(loaders) else None

    def request_version_loader_profile(self, vanilla_version: str, loader_version: str) -> dict:
        return self.request_fabric_meta(f"versions/loader/{vanilla_version}/{loader_version}/profile/json")

    def request_fabric_loader_versions(self) -> Iterator[str]:
        loaders = self.request_fabric_meta("versions/loader")
        return map(lambda obj: obj["version"], loaders)


FABRIC_API = FabricApi("fabric", "https://meta.fabricmc.net/v2/")
QUILT_API = FabricApi("quilt", "https://meta.quiltmc.org/v3/")


class FabricRoot:
    """Represent the root fabric version to load. The task `FabricInitTask` will only
    trigger if such state is present.
    """

    def __init__(self, api: FabricApi, vanilla_version: str, loader_version: Optional[str], prefix: str) -> None:
        self.api = api
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version
        self.prefix = prefix

    @classmethod
    def with_fabric(cls, vanilla_version: str, loader_version: Optional[str], prefix: str = "fabric") -> "FabricRoot":
        """Construct a root for resolving a Fabric version.
        """
        return cls(FABRIC_API, vanilla_version, loader_version, prefix)

    @classmethod
    def with_quilt(cls, vanilla_version: str, loader_version: Optional[str], prefix: str = "quilt") -> "FabricRoot":
        """Construct a root for resolving a Quilt version.
        """
        return cls(QUILT_API, vanilla_version, loader_version, prefix)


class FabricInitTask(Task):
    """This task loads metadata for a fabric version.

    :in FabricRoot: Optional, the fabric version to load if present.
    :in VersionRepositories: Used to register the fabric's version repository.
    :out MetadataRoot: The root version to load, for metadata task.
    """

    def execute(self, state: State, watcher: Watcher) -> None:

        root = state.get(FabricRoot)
        if root is None:
            return
        
        vanilla_version = root.vanilla_version
        loader_version = root.loader_version

        if loader_version is None:

            watcher.handle(FabricResolveEvent(root.api, vanilla_version, None))

            try:
                loader_version = root.api.request_fabric_loader_version(vanilla_version)
            except HttpError as error:
                if error.res.status not in (404, 400):
                    raise
                loader_version = None
            
            if loader_version is None:
                # Correct error if the error is just a not found.
                raise VersionNotFoundError(f"{root.prefix}-{vanilla_version}-???")

            watcher.handle(FabricResolveEvent(root.api, vanilla_version, loader_version))

        # Update the root version id to a valid one (without :).
        version_id = f"{root.prefix}-{vanilla_version}-{loader_version}"

        state.insert(MetadataRoot(version_id))
        state[VersionRepositories].insert(version_id, FabricRepository(root.api, vanilla_version, loader_version))


class FabricRepository(VersionRepository):
    """Internal class used as instance mapped to the fabric version.
    """

    def __init__(self, api: FabricApi, vanilla_version: str, loader_version: str) -> None:
        self.api = api
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version
    
    def fetch_version(self, version: Version, state: State) -> None:

        try:
            version.metadata = self.api.request_version_loader_profile(self.vanilla_version, self.loader_version)
        except HttpError as error:
            if error.res.status not in (404, 400):
                raise
            # Correct error if the error is just a not found.
            raise VersionNotFoundError(version.id)
            
        version.metadata["id"] = version.id
        version.write_metadata_file()


class FabricResolveEvent:
    """Event triggered when the loader version is missing and is being resolved.
    """
    __slots__ = "api", "vanilla_version", "loader_version"
    def __init__(self, api: FabricApi, vanilla_version: str, loader_version: Optional[str]) -> None:
        self.api = api
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version


def add_fabric_tasks(seq: Sequence) -> None:
    """Add tasks to a sequence for installing and running a Fabric mod loader version.

    The fabric tasks will run if the `FabricRoot` state is present, in such case a 
    `MetadataRoot` will be created if version resolution succeed.

    :param seq: The sequence to alter and add tasks to.
    """
    seq.prepend_task(FabricInitTask(), before=MetadataTask)


def _make_base_sequence(*,
    run: bool = False,
    context: Optional[Context] = None,
    version_manifest: Optional[VersionManifest] = None,
) -> Sequence:
    """Internal function for `make_<fabric|quilt>_sequence` functions.
    """
    
    from .vanilla import add_vanilla_tasks

    seq = Sequence()
    add_vanilla_tasks(seq, run=run)
    add_fabric_tasks(seq)

    seq.state.insert(context or Context())
    seq.state.insert(version_manifest or VersionManifest())

    return seq


def make_fabric_sequence(vanilla_version: str, loader_version: Optional[str] = None, *,
    run: bool = False,
    context: Optional[Context] = None,
    version_manifest: Optional[VersionManifest] = None,
    prefix: str = "fabric"
) -> Sequence:
    """Shortcut version of `add_vanilla_tasks` followed by `add_fabric_tasks` that 
    construct the sequence for you and add all the required state to get fabric installing
    and running.
    """

    seq = _make_base_sequence(run=run, context=context, version_manifest=version_manifest)
    seq.state.insert(FabricRoot.with_fabric(vanilla_version, loader_version, prefix))
    return seq


def make_quilt_sequence(vanilla_version: str, loader_version: Optional[str] = None, *,
    run: bool = False,
    context: Optional[Context] = None,
    version_manifest: Optional[VersionManifest] = None,
    prefix: str = "quilt"
) -> Sequence:
    """Shortcut version of `add_vanilla_tasks` followed by `add_fabric_tasks` that 
    construct the sequence for you and add all the required state to get quilt installing
    and running.
    """

    seq = _make_base_sequence(run=run, context=context, version_manifest=version_manifest)
    seq.state.insert(FabricRoot.with_quilt(vanilla_version, loader_version, prefix))
    return seq
