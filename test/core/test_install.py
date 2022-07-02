from portablemc import Context, Version, Start, StartOptions
from os import path
from pathlib import Path
import pytest
import shutil


class NoValidationVersion(Version):

    def _validate_version_meta(self, version_id: str, version_dir: str, version_meta_file: str, version_meta: dict) -> bool:
        return True  # To avoid fetching the online manifest


@pytest.mark.parametrize("version", ["b1.8.1", "1.5.2", "1.7.10", "1.16.5", "1.17.1", "1.19"])
def test_install(tmp_path, version):

    print(f"testing install in {tmp_path}")

    version_dir = tmp_path / "versions" / version
    version_dir.mkdir(parents=True, exist_ok=True)

    current_path = Path(path.dirname(__file__)).parent / "data" / "versions" / f"{version}.json"

    shutil.copy(str(current_path), str(version_dir))

    ctx = Context(str(tmp_path))
    ver = NoValidationVersion(ctx, version)

    ver.prepare_meta()
    ver.prepare_jar()
    ver.prepare_assets()
    ver.prepare_logger()
    ver.prepare_libraries()
    ver.prepare_jvm()

    assert (tmp_path / "assets" / "indexes" / f"{ver.assets_index_version}.json").is_file()
    assert (tmp_path / "jvm").is_dir()

    start = Start(ver)
    start.prepare(StartOptions())
