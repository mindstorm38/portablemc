from pathlib import Path
import shutil
import pytest

from portablemc.task import Sequence
from portablemc.vanilla import Context, VersionRepository, make_vanilla_sequence, \
    DownloadTask, AssetsFinalizeTask, \
    VersionManifest


def _test_prepare_execute(seq: Sequence):
    seq.remove_task(DownloadTask)
    seq.remove_task(AssetsFinalizeTask)  # Remove it because it needs downloading before.
    seq.execute()


@pytest.mark.parametrize("version", ["b1.8.1", "1.5.2", "1.7.10", "1.16.5", "1.17.1", "1.18.1.nopath", "1.19"])
def test_prepare_specific(tmp_context: Context, version: str):

    current_path = Path(__file__).parent.joinpath("data", "versions", f"{version}.json")
    version_dir = tmp_context.get_version(version).dir
    version_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy(current_path, version_dir)

    seq = make_vanilla_sequence(version, context=tmp_context, default_repository=VersionRepository())
    _test_prepare_execute(seq)


@pytest.mark.slow
def test_prepare_vanilla(tmp_context: Context, vanilla_version: str):
    """This test only run if --runslow argument is used and is used to check that all 
    major versions (including old beta/alpha) can be successfully parsed and prepared.
    """

    # Cache version manifest between tests.
    manifest = VersionManifest(tmp_context.work_dir / "version_manifest.json")

    seq = make_vanilla_sequence(vanilla_version, context=tmp_context, default_repository=manifest)
    _test_prepare_execute(seq)
