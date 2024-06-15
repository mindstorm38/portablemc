"""Definition of tasks for installing and running Fabric/Quilt mod loader.
"""

from .standard import Context, VersionHandle, Version, Watcher, VersionNotFoundError
from .http import http_request, HttpError

from typing import Optional, Any, Iterator


class _FabricApiLoader:
    """This class describes a loader returned from the fabric API. (unstable API)
    """
    __slots__ = "version", "stable"
    def __init__(self, version: str, stable: bool) -> None:
        self.version = version
        self.stable = stable


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

    def request_version_loader_profile(self, vanilla_version: str, loader_version: str) -> dict:
        """Return the version profile for the given vanilla version and loader.
        """
        return self.request_fabric_meta(f"versions/loader/{vanilla_version}/{loader_version}/profile/json")

    def _request_loaders(self, vanilla_version: Optional[str] = None) -> Iterator[_FabricApiLoader]:
        """Return an iterator of loaders available for the given vanilla version, if no
        vanilla version is specified, this returned an iterator of all loaders.
        """
        
        def map_loader(obj) -> _FabricApiLoader:
            return _FabricApiLoader(str(obj.get("version", "")), bool(obj.get("stable", False)))

        if vanilla_version is not None:
            loaders = self.request_fabric_meta(f"versions/loader/{vanilla_version}")
            return map(lambda obj: map_loader(obj["loader"]), loaders)
        else:
            return map(map_loader, self.request_fabric_meta("versions/loader"))

    def _request_latest_loader(self, vanilla_version: Optional[str] = None) -> Optional[_FabricApiLoader]:
        """Return the latest loader version for the given vanilla version, if no vanilla
        version is specified, this return the latest loader.
        """
        try:
            return next(self._request_loaders(vanilla_version))
        except StopIteration:
            return None

    # DEPRECATED:
    
    def request_fabric_loader_versions(self) -> Iterator[str]:
        """ deprecated, will be replaced by request_loaders """
        return map(lambda loader: loader.version, self._request_loaders())

    def request_fabric_loader_version(self, vanilla_version: str) -> Optional[str]:
        """ deprecated, will be replaced by request_latest_loader """
        loader = self._request_latest_loader(vanilla_version)
        return None if loader is None else loader.version


FABRIC_API = FabricApi("fabric", "https://meta.fabricmc.net/v2/")
QUILT_API = FabricApi("quilt", "https://meta.quiltmc.org/v3/")
LEGACYFABRIC_API = FabricApi("legacyfabric", "https://meta.legacyfabric.net/v2/")
BABRIC_API = FabricApi("babric", "https://meta.babric.glass-launcher.net/v2/")


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

    @classmethod
    def _with_legacyfabric(cls, vanilla_version: str = "release", loader_version: Optional[str] = None, *,
        context: Optional[Context] = None,
        prefix="legacyfabric"
    ) -> "FabricVersion":
        """Construct a root for resolving a LegacyFabric version"""
        return cls(LEGACYFABRIC_API, vanilla_version, loader_version, prefix, context=context)

    @classmethod
    def _with_babric(cls, vanilla_version: str = "release", loader_version: Optional[str] = None, *,
        context: Optional[Context] = None,
        prefix="babric"
    ) -> "FabricVersion":
        """Construct a root for resolving a LegacyFabric version"""
        return cls(BABRIC_API, vanilla_version, loader_version, prefix, context=context)

    def _resolve_version(self, watcher: Watcher) -> None:
        
        # Vanilla version may be "release" or "snapshot"
        self.vanilla_version = self.manifest.filter_latest(self.vanilla_version)[0]

        # Resolve loader version if not specified.
        if self.loader_version is None:

            watcher.handle(FabricResolveEvent(self.api, self.vanilla_version, None))

            try:
                loader = self.api._request_latest_loader(self.vanilla_version)
                self.loader_version = None if loader is None else loader.version
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
