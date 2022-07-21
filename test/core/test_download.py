from portablemc import DownloadList, DownloadEntry, DownloadReport
from os import path


def test_download(tmp_path):

    assets_dir = tmp_path / "assets"

    default = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                            str(assets_dir / "icons" / "icon_16x16.png"),
                            name="default")

    check_sha1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               str(assets_dir / "icons" / "icon_16x16_check_sha1.png"),
                               sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               name="check_sha1")

    check_size = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               str(assets_dir / "icons" / "icon_16x16_check_size.png"),
                               size=3665,
                               name="check_size")

    check_all = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                              str(assets_dir / "icons" / "icon_16x16_check_all.png"),
                              sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                              size=3665,
                              name="check_all")

    wrong_sha1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               str(assets_dir / "icons" / "icon_16x16_wrong_sha1.png"),
                               sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510b",
                               name="wrong_sha1")

    wrong_size = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               str(assets_dir / "icons" / "icon_16x16_wrong_size.png"),
                               size=1189,
                               name="wrong_size")

    not_found = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
                              str(assets_dir / "icons" / "icon_16x16_not_found.png"),
                              sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                              size=3665,
                              name="not_found")

    conn_err = DownloadEntry("https://rfdfdfesources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
                             str(assets_dir / "icons" / "icon_16x16_not_found.png"),
                             sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                             size=3665,
                             name="conn_err")

    sacrificial0 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                             str(assets_dir / "icons" / "icon_16x16_sacrificial0.png"),
                             name="sacrificial0")

    sacrificial1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                             str(assets_dir / "icons" / "icon_16x16_sacrificial1.png"),
                             name="sacrificial1")

    # This error should have overwritten the file
    wrong_sha1_sacrificial0 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                             str(assets_dir / "icons" / "icon_16x16_sacrificial0.png"),
                             sha1="bdf48ef6b5d0d23bbb02e17d04865216179f510b",
                             name="wrong_sha1_sacrificial0")

    # This error should have overwritten the file
    wrong_size_sacrificial1 = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                               str(assets_dir / "icons" / "icon_16x16_sacrificial1.png"),
                               size=1189,
                               name="wrong_size_sacrificial1")

    not_sacrificial = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a",
                             str(assets_dir / "icons" / "icon_16x16_not_sacrificial.png"),
                             name="not_sacrificial")

    # This error should not overwrite de file
    not_found_not_sacrificial = DownloadEntry("https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
                             str(assets_dir / "icons" / "icon_16x16_not_sacrificial.png"),
                             name="not_found_not_sacrificial")

    # This error should not overwrite de file
    conn_err_not_sacrificial = DownloadEntry("https://rfdfdfesources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216",
                             str(assets_dir / "icons" / "icon_16x16_not_sacrificial.png"),
                             name="conn_err_not_sacrificial")

    dl = DownloadList()

    dl.append(default)
    dl.append(check_sha1)
    dl.append(check_size)
    dl.append(check_all)
    dl.append(wrong_sha1)
    dl.append(wrong_size)
    dl.append(not_found)
    dl.append(conn_err)

    dl.append(sacrificial0)
    dl.append(sacrificial1)
    dl.append(wrong_sha1_sacrificial0)
    dl.append(wrong_size_sacrificial1)

    dl.append(not_sacrificial)
    dl.append(not_found_not_sacrificial)
    dl.append(conn_err_not_sacrificial)

    report = dl.download_files()

    print(report.fails)

    assert report.fails[wrong_sha1] == DownloadReport.INVALID_SHA1
    assert report.fails[wrong_size] == DownloadReport.INVALID_SIZE
    assert report.fails[not_found] == DownloadReport.NOT_FOUND
    assert report.fails[conn_err] == DownloadReport.CONN_ERROR
    assert len(report.fails) == 8

    assert path.isfile(default.dst)
    assert path.isfile(check_sha1.dst)
    assert path.isfile(check_size.dst)
    assert path.isfile(check_all.dst)
    assert path.isfile(not_sacrificial.dst)
    assert not path.isfile(wrong_sha1.dst)
    assert not path.isfile(wrong_size.dst)
    assert not path.isfile(not_found.dst)
    assert not path.isfile(conn_err.dst)
    assert not path.isfile(sacrificial0.dst)
    assert not path.isfile(sacrificial1.dst)
