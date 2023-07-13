"""Definition of tasks for installing and running Fabric/Quilt mod loader.
"""

from .vanilla import MetadataRoot, MetadataTask, VersionRepository, Version, \
    VersionRepositories
from .task import Task, State, Watcher, Sequence
from .http import http_request

from typing import Optional, Any


class FabricRoot:
    """Represent the root fabric version to load. The task `FabricInitTask` will only
    trigger if such state is present.
    """

    def __init__(self, vanilla_version: str, loader_version: Optional[str]) -> None:
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version


class FabricInitTask(Task):
    """This task loads metadata for a fabric version.

    :in VersionRoot: The root version to load. If this version's id follow the following
    format `fabric:[<mc-version>[:<loader-version>]]`, then this task will trigger and
    prepare the fabric's metadata.
    """

    def __init__(self, prefix: str, api: str) -> None:
        self.prefix = prefix
        self.api = api

    def execute(self, state: State, watcher: Watcher) -> None:

        root = state.get(FabricRoot)
        if root is None:
            return
        
        vanilla_version = root.vanilla_version
        loader_version = root.loader_version

        print(f"{vanilla_version=} {loader_version=}")

        if loader_version is None:
            loader_version = request_fabric_loader_version(vanilla_version)

        # Update the root version id to a valid one (without :).
        version_id = f"{self.prefix}-{vanilla_version}-{loader_version}"
        state.insert(MetadataRoot(version_id))
        state[VersionRepositories].insert(version_id, FabricRepository(version_id, vanilla_version, loader_version))


class FabricRepository(VersionRepository):
    """Internal class used as instance mapped to the fabric version.
    """

    def __init__(self, version_id: str, vanilla_version: str, loader_version: str) -> None:
        self.version_id = version_id
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version

    def validate_version_meta(self, version: Version) -> bool:
        assert version.id == self.version_id, "should not trigger for this version"
        return True
    
    def fetch_version_meta(self, version: Version) -> None:
        version.metadata = request_version_loader_profile(self.vanilla_version, self.loader_version)
        version.metadata["id"] = self.version_id
        version.write_metadata_file()


def request_fabric_meta(method: str) -> Any:
    """Generic HTTP request to the fabric's REST API.
    """
    return http_request("GET", f"https://meta.fabricmc.net/{method}", accept="application/json").json()

def request_fabric_loader_version(vanilla_version: str) -> str:
    return request_fabric_meta(f"v2/versions/loader/{vanilla_version}")[0].get("loader", {}).get("version")

def request_version_loader_profile(vanilla_version: str, loader_version: str) -> dict:
    return request_fabric_meta(f"v2/versions/loader/{vanilla_version}/{loader_version}/profile/json")


def alter_fabric_sequence(seq: Sequence, *, prefix: str = "fabric") -> None:
    """Alter a sequence for installing and running a Fabric mod loader version.

    The fabric tasks will run if the `FabricRoot` state is present, in such case a 
    `MetadataRoot` will be created if version resolution succeed.

    :param seq: The sequence to alter and add
    """
    seq.prepend_task(FabricInitTask(prefix, "https://meta.fabricmc.net/v2/"), before=MetadataTask)


def alter_quilt_sequence(seq: Sequence, *, prefix: str = "quilt") -> None:
    seq.prepend_task(FabricInitTask(prefix, "https://meta.quiltmc.org/v3/"), before=MetadataTask)
