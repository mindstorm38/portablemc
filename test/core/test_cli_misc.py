
def test_format_locale_date():

    from portablemc.cli import format_locale_date

    assert isinstance(format_locale_date("2022-06-23T17:01:27+00:00"), str)
    assert isinstance(format_locale_date(1656164315.0), str)


def test_format_number():

    from portablemc.cli import format_number

    assert format_number(0) == "0"
    assert format_number(999) == "999"
    assert format_number(1000) == "1.0k"
    assert format_number(999999) == "999.9k"
    assert format_number(1000000) == "1.0M"
    assert format_number(999999999) == "999.9M"
    assert format_number(1000000000) == "1.0G"
    assert format_number(1000000000000) == "1000.0G"


def test_ellipsis_str():

    from portablemc.cli import ellipsis_str

    assert ellipsis_str("abc", 4) == "abc"
    assert ellipsis_str("abcd", 4) == "abcd"
    assert ellipsis_str("abcde", 4) == "a..."
