"""Management of the official Mojang's version manifest. It's used to
download and check for up-to-date Minecraft vanilla versions.
"""

from pathlib import Path
import json

from .http import json_request

from typing import Optional, Tuple


VERSION_MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


class VersionManifest:
    """The Mojang's official version manifest. Providing officially
    available versions with optional cache file.
    """

    def __init__(self, 
        cache_file: Optional[Path] = None, 
        cache_timeout: Optional[float] = None
    ) -> None:
        self.data: Optional[dict] = None
        self.cache_timeout = cache_timeout
        self.cache_file = cache_file
        self.sync = False

    def _ensure_data(self) -> dict:

        if self.data is None:

            headers = {}
            cache_data = None

            # If a cache file should be used, try opening it and 
            if self.cache_file is not None:
                try:
                    with self.cache_file.open("rt") as cache_fp:
                        cache_data = json.load(cache_fp)
                    if "last_modified" in cache_data:
                        headers["If-Modified-Since"] = cache_data["last_modified"]
                except (OSError, json.JSONDecodeError):
                    pass

            rcv_headers = {}
            status, data = (404, {})

            if self.cache_timeout is None or self.cache_timeout > 0:
                try:
                    status, data = json_request(
                        VERSION_MANIFEST_URL, "GET", 
                        headers=headers, ignore_error=True, 
                        timeout=self.cache_timeout, 
                        rcv_headers=rcv_headers)
                except OSError:
                    # We silently ignore OSError (all socket errors 
                    # and URL errors) and use default 404
                    pass

            if status == 200:
                if "Last-Modified" in rcv_headers:
                    data["last_modified"] = rcv_headers["Last-Modified"]
                self.data = data
                self.sync = True
                if self.cache_file is not None:
                    self.cache_file.parent.mkdir(parents=True, exist_ok=True)
                    with self.cache_file.open("wt") as cache_fp:
                        json.dump(data, cache_fp, indent=2)
            else:
                # If the status is not 200, we fall back to the cached
                # data if it exists, if not, raise error. 
                # This can be 304 status, in this case cache_data is 
                # set so there is no problem.
                if cache_data is None:
                    raise VersionManifestError
                self.data = cache_data

        return self.data

    def filter_latest(self, version: str) -> Tuple[str, bool]:
        if version in ("release", "snapshot"):
            latest = self._ensure_data()["latest"].get(version)
            if latest is not None:
                return latest, True
        return version, False

    def get_version(self, version: str) -> Optional[dict]:
        version, _alias = self.filter_latest(version)
        try:
            for version_data in self._ensure_data()["versions"]:
                if version_data["id"] == version:
                    return version_data
        except VersionManifestError:
            # Silently ignore manifest errors because we want to be 
            # able to launch offline.
            pass
        return None

    def all_versions(self) -> list:
        return self._ensure_data()["versions"]

    def get_version_type(self, version: str) -> str:
        obj = self.get_version(version)
        return "release" if obj is None else obj.get("type", "release")


class VersionManifestError(Exception):
    pass
