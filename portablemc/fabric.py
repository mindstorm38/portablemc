"""Definition of tasks for installing and running Fabric/Quilt mod loader.
"""

from portablemc.standard import VersionHandle, Watcher
from .standard import Context, VersionHandle, Version, Watcher, VersionNotFoundError
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


class FabricVersion(Version):

    def __init__(self, context: Context, api: FabricApi, vanilla_version: str, loader_version: Optional[str], prefix: str) -> None:
        
        super().__init__(context, "")  # Do not give a root version for now.

        self._api = api
        self._vanilla_version = vanilla_version
        self._loader_version = loader_version
        self._prefix = prefix

    @classmethod
    def with_fabric(cls, context: Context, vanilla_version: str, loader_version: Optional[str], prefix: str = "fabric") -> "FabricVersion":
        """Construct a root for resolving a Fabric version.
        """
        return cls(context, FABRIC_API, vanilla_version, loader_version, prefix)

    @classmethod
    def with_quilt(cls, context: Context, vanilla_version: str, loader_version: Optional[str], prefix: str = "quilt") -> "FabricVersion":
        """Construct a root for resolving a Quilt version.
        """
        return cls(context, QUILT_API, vanilla_version, loader_version, prefix)

    def _resolve_version(self, watcher: Watcher) -> None:

        # Vanilla version may be "release" or "snapshot"
        self._vanilla_version = self._manifest.filter_latest(self._vanilla_version)[0]
        
        # Resolve loader version if not specified.
        if self._loader_version is None:

            watcher.handle(FabricResolveEvent(self._api, self._vanilla_version, None))

            try:
                self._loader_version = self._api.request_fabric_loader_version(self._vanilla_version)
            except HttpError as error:
                if error.res.status not in (404, 400):
                    raise
                self._loader_version = None
            
            if self._loader_version is None:
                # Correct error if the error is just a not found.
                raise VersionNotFoundError(f"{self._prefix}-{self._vanilla_version}-???")

            watcher.handle(FabricResolveEvent(self._api, self._vanilla_version, self._loader_version))
        
        # Finally define the full version id.
        self._version = f"{self._prefix}-{self._vanilla_version}-{self._loader_version}"

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        if version.id == self._version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:

        if version.id != self._version:
            return super()._fetch_version(version, watcher)
        
        assert self._loader_version is not None, "_resolve_fabric_loader(...) missing"
        
        try:
            version.metadata = self._api.request_version_loader_profile(self._vanilla_version, self._loader_version)
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
