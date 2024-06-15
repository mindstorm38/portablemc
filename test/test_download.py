from pathlib import Path
from os import path
import pytest

from portablemc.download import DownloadEntry, DownloadList, \
    DownloadResult, DownloadResultError


def test_download(tmp_path):

    assets_dir = tmp_path / "assets"

    default = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16.png",
        name="default")

    check_sha1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16_check_sha1.png",
        sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        name="check_sha1")

    check_size = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16_check_size.png",
        size=3665,
        name="check_size")

    check_all = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16_check_all.png",
        sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        size=3665,
        name="check_all")

    wrong_sha1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16_wrong_sha1.png",
        sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510b",
        name="wrong_sha1")

    wrong_size = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        assets_dir / "icons" / "icon_16x16_wrong_size.png",
        size=1189,
        name="wrong_size")

    not_found = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
        assets_dir / "icons" / "icon_16x16_not_found.png",
        sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        size=3665,
        name="not_found")

    conn_err = DownloadEntry("https://rfdfdfesources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
        assets_dir / "icons" / "icon_16x16_conn_err.png",
        sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
        size=3665,
        name="conn_err")

    dl = DownloadList()

    dl.add(default)
    dl.add(check_sha1)
    dl.add(check_size)
    dl.add(check_all)
    dl.add(wrong_sha1)
    dl.add(wrong_size)
    dl.add(not_found)
    dl.add(conn_err)

    with pytest.raises(ValueError):
        dl.add(DownloadEntry("ssh://foo.bar", Path("invalid")))

    results = {result.entry: result for result_count, result in dl.download(2)}

    def is_error(result: DownloadResult, code: str) -> bool:
        return isinstance(result, DownloadResultError) and result.code == code

    assert len(results) == 8
    
    assert is_error(results[wrong_sha1], DownloadResultError.INVALID_SHA1)
    assert is_error(results[wrong_size], DownloadResultError.INVALID_SIZE)
    assert is_error(results[not_found], DownloadResultError.NOT_FOUND)
    assert is_error(results[conn_err], DownloadResultError.CONNECTION)

    assert path.isfile(default.dst)
    assert path.isfile(check_sha1.dst)
    assert path.isfile(check_size.dst)
    assert path.isfile(check_all.dst)
    assert not path.isfile(wrong_sha1.dst)
    assert not path.isfile(wrong_size.dst)
    assert not path.isfile(not_found.dst)
    assert not path.isfile(conn_err.dst)
