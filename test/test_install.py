"""Functional tests of the game's installation.
"""

from pathlib import Path
import shutil
import pytest

from portablemc.standard import Context, Version, VersionManifest, TooMuchParentsError, \
    JarNotFoundError
from portablemc.download import DownloadList
from portablemc.fabric import FabricVersion
from portablemc.forge import ForgeVersion, _NeoForgeVersion

from typing import Type, Optional


def _remove_assets(version: Version):

    # We want to avoid download all assets since it can take really long time, and it
    # test nothing new, so we temporarily replace the download list with a trash one.
    _old_resolve_assets = version._resolve_assets
    def _test_resolve_assets(watcher) -> None:
        saved_dl = version._dl
        version._dl = DownloadList() 
        _old_resolve_assets(watcher)
        version._dl = saved_dl

    version._resolve_assets = _test_resolve_assets
    version._finalize_assets = lambda watcher: None


@pytest.mark.parametrize("test_version", ["b1.8.1", "1.5.2", "1.7.10", "1.16.5", "1.17.1", "1.18.1.nopath", "1.19"])
def test_install_specific(tmp_context: Context, test_version: str):

    test_version_id = f"test-{test_version}"

    current_path = Path(__file__).parent.joinpath("data", "versions", f"{test_version}.json")
    handle = tmp_context.get_version(test_version_id)
    handle.dir.mkdir(parents=True, exist_ok=True)
    shutil.copy(current_path, handle.metadata_file())
    assert handle.metadata_exists()

    version = Version(test_version_id, context=tmp_context)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.parametrize("test_version", ["0.14.22", None])
def test_install_fabric(tmp_context: Context, test_version: Optional[str]):
    version = FabricVersion.with_fabric("1.20.1", test_version, context=tmp_context)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.parametrize("test_version", ["0.20.0-beta.11", None])
def test_install_quilt(tmp_context: Context, test_version: Optional[str]):
    version = FabricVersion.with_quilt("1.20.1", test_version, context=tmp_context)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.parametrize("test_version", ["1.5.2-7.8.1.738", "1.12.2-14.23.5.2847", "1.12.2-14.23.5.2851", "1.20.1-47.1.0", "1.20.1-latest"])
def test_install_forge(tmp_context: Context, test_version: str):
    """Testing forge install for both old an new formats.
    """

    version = ForgeVersion(test_version)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.parametrize("test_version", ["1.20.1", "1.20.1-47.1.79", "1.21", "21.0.42-beta"])
def test_install_neoforge(tmp_context: Context, test_version: str):
    """Testing neoforge install for both old an new formats.
    """

    version = _NeoForgeVersion(test_version)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.slow
def test_install_vanilla(tmp_context: Context, vanilla_version: str):
    """This test only run if --runslow argument is used and is used to check that all 
    major versions (including old beta/alpha) can be successfully parsed and prepared.
    """

    version = Version(vanilla_version, context=tmp_context)
    version.manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")
    _remove_assets(version)
    version.install()


@pytest.mark.parametrize("test_version,test_error,test_match", [
    ("err.recurse", TooMuchParentsError, None),
    ("err.jar_not_found", JarNotFoundError, None),
])
def test_install_error(tmp_context: Context, 
    test_version: str,
    test_error: Type[Exception],
    test_match: Optional[str]):
    
    current_path = Path(__file__).parent.joinpath("data", "versions", f"{test_version}.json")
    handle = tmp_context.get_version(test_version)
    handle.dir.mkdir(parents=True, exist_ok=True)
    shutil.copy(current_path, handle.metadata_file())
    assert handle.metadata_exists()

    version = Version(test_version, context=tmp_context)
    # Avoid downloading in cases where errors has not been raised.
    version._download = lambda watcher: None

    with pytest.raises(test_error, match=test_match):
        version.install()
