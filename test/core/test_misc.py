
def test_sha1():

    from portablemc import calc_input_sha1
    from io import BytesIO

    assert calc_input_sha1(BytesIO(b"hello world!")) == "430ce34d020724ed75a196dfc2ad67c77772d169"
    assert calc_input_sha1(BytesIO(b"hello world!"), buffer_len=2) == "430ce34d020724ed75a196dfc2ad67c77772d169"


def test_iso_date():

    from portablemc import from_iso_date
    from datetime import datetime, timezone, timedelta

    date = from_iso_date("2022-06-23T17:01:27+00:00")
    assert date == datetime(2022, 6, 23, 17, 1, 27, 0, timezone(timedelta()))

    date = from_iso_date("2012-03-01T22:00:00+05:00")
    assert date == datetime(2012, 3, 1, 22, 0, 0, 0, timezone(timedelta(hours=5)))


def test_merge():

    from portablemc import merge_dict

    dst = {}
    merge_dict(dst, {})
    assert dst == {}

    dst = {}
    merge_dict(dst, {"ok": 65, "lst": [True, 43], "dct": {"foo": "bar"}})
    assert dst == {"ok": 65, "lst": [True, 43], "dct": {"foo": "bar"}}

    dst = {"ok": 32, "lst": [2.3]}
    merge_dict(dst, {"ok": 65, "lst": [True, 43], "dct": {"foo": {"bar": "baz"}}})
    assert dst == {"ok": 32, "lst": [2.3, True, 43], "dct": {"foo": {"bar": "baz"}}}


def test_replace_vars():

    from portablemc import replace_vars, replace_list_vars

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


def test_can_extract_native():

    from portablemc import can_extract_native

    assert can_extract_native("foo.so")
    assert can_extract_native("foo.dll")
    assert can_extract_native("foo.dylib")
    assert not can_extract_native("foo.other")
