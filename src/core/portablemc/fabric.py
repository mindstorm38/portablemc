"""Definition of tasks for installing and running Fabric mod loader.
"""

from .task import Task, State, Watcher, Sequence
from .vanilla import VersionRoot, MetadataTask
from .http import http_request

from typing import Any


class FabricMetadataTask(Task):
    """This task loads metadata for a fabric version.

    :in VersionRoot: The root version to load. If this version's id follow the following
    format `fabric:[<mc-version>[:<loader-version>]]`, then this task will trigger and
    prepare the fabric's metadata.
    """

    def execute(self, state: State, watcher: Watcher) -> None:
        
        version_id = state[VersionRoot].id

        parts = version_id.split(":")
        if len(parts) < 2 or parts[1] != "fabric":
            return
        

        pass

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
    seq.prepend_task(FabricMetadataTask(), before=MetadataTask)
