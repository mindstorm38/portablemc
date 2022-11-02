
def test_format_locale_date():

    from portablemc.cli import format_locale_date

    assert isinstance(format_locale_date("2022-06-23T17:01:27+00:00"), str)
    assert isinstance(format_locale_date(1656164315.0), str)


def test_format_number():

    from portablemc.cli import format_number, format_bytes

    assert format_number(0) == "0"
    assert format_number(999) == "999"
    assert format_number(1000) == "1.0k"
    assert format_number(999999) == "999.9k"
    assert format_number(1000000) == "1.0M"
    assert format_number(999999999) == "999.9M"
    assert format_number(1000000000) == "1.0G"
    assert format_number(1000000000000) == "1000.0G"

    assert format_bytes(0) == "0B"
    assert format_bytes(999) == "999B"
    assert format_bytes(1000) == "1.0kB"
    assert format_bytes(999999) == "999.9kB"
    assert format_bytes(1000000) == "1.0MB"
    assert format_bytes(999999999) == "999.9MB"
    assert format_bytes(1000000000) == "1.0GB"
    assert format_bytes(1000000000000) == "1000.0GB"


def test_ellipsis_str():

    from portablemc.cli import ellipsis_str

    assert ellipsis_str("abc", 4) == "abc"
    assert ellipsis_str("abcd", 4) == "abcd"
    assert ellipsis_str("abcde", 4) == "a..."


def test_anonymise_email():
    from portablemc.cli import anonymise_email
    assert anonymise_email("foo.bar@baz.com") == "f*****r@b*z.com"


def test_register_arguments():

    from portablemc.cli import register_arguments

    # Ensure that the arguments registering successfuly works.
    register_arguments()


def test_library_specifier_filter():
    
    from portablemc.cli import LibrarySpecifierFilter
    from portablemc import LibrarySpecifier

    assert str(LibrarySpecifierFilter("baz", None, None)) == "baz:"
    assert str(LibrarySpecifierFilter("baz", "0.1.0", None)) == "baz:0.1.0"
    assert str(LibrarySpecifierFilter("baz", "0.1.0", "natives-windows-x86")) == "baz:0.1.0:natives-windows-x86"
    assert str(LibrarySpecifierFilter("baz", None, "natives-windows-x86")) == "baz::natives-windows-x86"

    spec = LibrarySpecifier("foo.bar", "baz", "0.1.0", None)
    spec_classified = LibrarySpecifier("foo.bar", "baz", "0.1.0", "natives-windows-x86")

    assert LibrarySpecifierFilter("baz", None, None).matches(spec)
    assert LibrarySpecifierFilter("baz", None, None).matches(spec_classified)
    assert not LibrarySpecifierFilter("nomatch", None, None).matches(spec)
    assert not LibrarySpecifierFilter("nomatch", None, None).matches(spec_classified)

    assert LibrarySpecifierFilter("baz", "0.1.0", None).matches(spec)
    assert LibrarySpecifierFilter("baz", "0.1.0", None).matches(spec_classified)
    assert not LibrarySpecifierFilter("baz", "0.2.0", None).matches(spec)
    assert not LibrarySpecifierFilter("baz", "0.2.0", None).matches(spec_classified)

    assert LibrarySpecifierFilter("baz", None, "natives-windows-x86").matches(spec_classified)
    assert LibrarySpecifierFilter("baz", None, "natives-windows").matches(spec_classified)
    assert LibrarySpecifierFilter("baz", None, "natives").matches(spec_classified)
    assert not LibrarySpecifierFilter("baz", None, "windows").matches(spec)
    assert not LibrarySpecifierFilter("baz", None, "windows").matches(spec_classified)

    assert LibrarySpecifierFilter("baz", "0.1.0", "natives-windows-x86").matches(spec_classified)
    assert LibrarySpecifierFilter("baz", "0.1.0", "natives-windows").matches(spec_classified)
    assert LibrarySpecifierFilter("baz", "0.1.0", "natives").matches(spec_classified)
    assert not LibrarySpecifierFilter("baz", "0.2.0", "natives-windows-x86").matches(spec_classified)
    assert not LibrarySpecifierFilter("baz", "0.1.0", "windows").matches(spec)
    assert not LibrarySpecifierFilter("baz", "0.1.0", "windows").matches(spec_classified)
