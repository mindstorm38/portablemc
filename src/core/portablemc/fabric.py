"""Definition of tasks for installing and running Fabric mod loader.
"""

from .vanilla import MetadataRoot, MetadataTask, VersionRepository, Version
from .task import Task, State, Watcher, Sequence
from .http import http_request

from typing import Optional, Any



class FabricRoot:
    """Represent the root fabric version to load. The task `FabricInitTask` will only
    trigger if such state is present.
    """

    def __init__(self, game_version: str, loader_version: Optional[str]) -> None:
        self.game_version = game_version
        self.loader_version = loader_version


class FabricInitTask(Task):
    """This task loads metadata for a fabric version.

    :in VersionRoot: The root version to load. If this version's id follow the following
    format `fabric:[<mc-version>[:<loader-version>]]`, then this task will trigger and
    prepare the fabric's metadata.
    """

    def __init__(self, prefix: str) -> None:
        self.prefix = prefix

    def execute(self, state: State, watcher: Watcher) -> None:

        root = state.get(FabricRoot)
        if root is None:
            return
        
        game_version = root.game_version
        loader_version = root.loader_version

        if loader_version is None:
            loader_version = request_fabric_loader_version(game_version)

        # Update the root version id to a valid one (without :).
        state[MetadataRoot].id = f"{self.prefix}-{game_version}-{loader_version}"
        state[MetadataRoot].repository = FabricRepository()


class FabricRepository(VersionRepository):

    def validate_version_meta(self, version: Version) -> bool:
        return super().validate_version_meta(version)
    
    def fetch_version_meta(self, version: Version) -> None:
        return super().fetch_version_meta(version)


def request_fabric_meta(method: str) -> Any:
    """Generic HTTP request to the fabric's REST API.
    """
    return http_request("GET", f"https://meta.fabricmc.net/{method}", accept="application/json").json()

def request_fabric_loader_version(game_version: str) -> str:
    return request_fabric_meta(f"v2/versions/loader/{game_version}")[0].get("loader", {}).get("version")

def request_version_loader_profile(game_version: str, loader_version: str) -> dict:
    return request_fabric_meta(f"v2/versions/loader/{game_version}/{loader_version}/profile/json")


def alter_fabric_sequence(seq: Sequence) -> None:
    """Alter a sequence for installing and running a Fabric mod loader version.

    :param seq: The sequence to alter and add
    """
    seq.prepend_task(FabricInitTask("fabric"), before=MetadataTask)
