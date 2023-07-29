import pytest


def test_sha1():

    from portablemc.util import calc_input_sha1
    from io import BytesIO

    assert calc_input_sha1(BytesIO(b"hello world!")) == "430ce34d020724ed75a196dfc2ad67c77772d169"
    assert calc_input_sha1(BytesIO(b"hello world!"), buffer_len=2) == "430ce34d020724ed75a196dfc2ad67c77772d169"


def test_iso_date():

    from portablemc.util import from_iso_date
    from datetime import datetime, timezone, timedelta

    date = from_iso_date("2022-06-23T17:01:27+00:00")
    assert date == datetime(2022, 6, 23, 17, 1, 27, 0, timezone(timedelta()))

    date = from_iso_date("2012-03-01T22:00:00+05:00")
    assert date == datetime(2012, 3, 1, 22, 0, 0, 0, timezone(timedelta(hours=5)))


def test_merge():

    from portablemc.util import merge_dict

    dst = {}
    merge_dict(dst, {})
    assert dst == {}

    dst = {}
    merge_dict(dst, {"ok": 65, "lst": [True, 43], "dct": {"foo": "bar"}})
    assert dst == {"ok": 65, "lst": [True, 43], "dct": {"foo": "bar"}}

    dst = {"ok": 32, "lst": [2.3], "dct": {"baz": True}}
    merge_dict(dst, {"ok": 65, "lst": [True, 43], "dct": {"foo": {"bar": "baz"}}})
    assert dst == {"ok": 32, "lst": [True, 43, 2.3], "dct": {"baz": True, "foo": {"bar": "baz"}}}


def test_replace_vars():

    from portablemc.standard import replace_vars, replace_list_vars

    assert replace_vars("this is foo value: ${foo}", {"foo": "89658"}) == "this is foo value: 89658"

    assert list(replace_list_vars([
        "this is foo value: ${foo}",
        "this is bar value: ${bar}!!!",
        "this is both values: ${foo}/${bar}...",
        "this is unknown key: ${unknown}"
    ], {"foo": "89658", "bar": "test"})) == [
        "this is foo value: 89658",
        "this is bar value: test!!!",
        "this is both values: 89658/test...",
        "this is unknown key: ${unknown}"
    ]


def test_library_specifier():

    from portablemc.util import LibrarySpecifier

    with pytest.raises(ValueError):
        LibrarySpecifier.from_str("foo.bar:baz")

    spec = LibrarySpecifier.from_str("foo.bar:baz:0.1.0")
    assert spec.group == "foo.bar"
    assert spec.artifact == "baz"
    assert spec.version == "0.1.0"
    assert str(spec) == "foo.bar:baz:0.1.0"
    assert spec.file_path() == "foo/bar/baz/0.1.0/baz-0.1.0.jar"

    spec = LibrarySpecifier.from_str("foo.bar:baz:0.1.0:classifier")
    assert spec.group == "foo.bar"
    assert spec.artifact == "baz"
    assert spec.version == "0.1.0"
    assert spec.classifier == "classifier"
    assert str(spec) == "foo.bar:baz:0.1.0:classifier"
    assert spec.file_path() == "foo/bar/baz/0.1.0/baz-0.1.0-classifier.jar"

    spec = LibrarySpecifier.from_str("foo.bar:baz:0.1.0:classifier@txt")
    assert spec.group == "foo.bar"
    assert spec.artifact == "baz"
    assert spec.version == "0.1.0"
    assert spec.classifier == "classifier"
    assert spec.extension == "txt"
    assert str(spec) == "foo.bar:baz:0.1.0:classifier@txt"
    assert spec.file_path() == "foo/bar/baz/0.1.0/baz-0.1.0-classifier.txt"
