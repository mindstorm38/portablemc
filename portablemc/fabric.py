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

    def __init__(self, api: FabricApi, vanilla_version: str, loader_version: Optional[str], 
        prefix: str, *,
        context: Optional[Context] = None,
    ) -> None:
        
        super().__init__("", context=context)  # Do not give a root version for now.

        self.api = api
        self.vanilla_version = vanilla_version
        self.loader_version = loader_version
        self.prefix = prefix

    @classmethod
    def with_fabric(cls, vanilla_version: str = "release", loader_version: Optional[str] = None, *,
        context: Optional[Context] = None, 
        prefix: str = "fabric"
    ) -> "FabricVersion":
        """Construct a root for resolving a Fabric version.
        """
        return cls(FABRIC_API, vanilla_version, loader_version, prefix, context=context)

    @classmethod
    def with_quilt(cls, vanilla_version: str = "release", loader_version: Optional[str] = None, *,
        context: Optional[Context] = None,
        prefix: str = "quilt"
    ) -> "FabricVersion":
        """Construct a root for resolving a Quilt version.
        """
        return cls(QUILT_API, vanilla_version, loader_version, prefix, context=context)

    def _resolve_version(self, watcher: Watcher) -> None:

        # Vanilla version may be "release" or "snapshot"
        self.vanilla_version = self.manifest.filter_latest(self.vanilla_version)[0]
        
        # Resolve loader version if not specified.
        if self.loader_version is None:

            watcher.handle(FabricResolveEvent(self.api, self.vanilla_version, None))

            try:
                self.loader_version = self.api.request_fabric_loader_version(self.vanilla_version)
            except HttpError as error:
                if error.res.status not in (404, 400):
                    raise
                self.loader_version = None
            
            if self.loader_version is None:
                # Correct error if the error is just a not found.
                raise VersionNotFoundError(f"{self.prefix}-{self.vanilla_version}-???")

            watcher.handle(FabricResolveEvent(self.api, self.vanilla_version, self.loader_version))
        
        # Finally define the full version id.
        self.version = f"{self.prefix}-{self.vanilla_version}-{self.loader_version}"

    def _load_version(self, version: VersionHandle, watcher: Watcher) -> bool:
        if version.id == self.version:
            return version.read_metadata_file()
        else:
            return super()._load_version(version, watcher)

    def _fetch_version(self, version: VersionHandle, watcher: Watcher) -> None:

        if version.id != self.version:
            return super()._fetch_version(version, watcher)
        
        assert self.loader_version is not None, "_resolve_fabric_loader(...) missing"
        
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
