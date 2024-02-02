
def test_format_locale_date():

    from portablemc.cli.util import format_locale_date

    assert isinstance(format_locale_date("2022-06-23T17:01:27+00:00"), str)
    assert isinstance(format_locale_date(1656164315.0), str)


def test_format_number():

    from portablemc.cli.util import format_number, format_duration

    assert format_number(0) == "0 "
    assert format_number(999) == "999 "
    assert format_number(1000) == "1.0 k"
    assert format_number(999999) == "999.9 k"
    assert format_number(1000000) == "1.0 M"
    assert format_number(999999999) == "999.9 M"
    assert format_number(1000000000) == "1.0 G"
    assert format_number(1000000000000) == "1000.0 G"

    assert format_duration(0) == "0 s"
    assert format_duration(59) == "59 s"
    assert format_duration(60) == "1 m"
    assert format_duration(119) == "1 m"
    assert format_duration(120) == "2 m"
    assert format_duration(3599) == "59 m"
    assert format_duration(3600) == "1 h"
    assert format_duration(7200) == "2 h"


def test_anonymise_email():
    from portablemc.cli.util import anonymize_email
    assert anonymize_email("foo.bar@baz.com") == "f*****r@b*z.com"


def test_library_specifier_filter():
    
    from portablemc.cli.util import LibrarySpecifierFilter
    from portablemc.util import LibrarySpecifier

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


def test_parser_and_completion():

    from portablemc.cli.complete import gen_zsh_completion, gen_bash_completion
    from portablemc.cli import register_arguments

    # Ensure that the arguments registering successfully works.
    args = register_arguments()
    # Just check that it doesn't crash.
    gen_zsh_completion(args)
    gen_bash_completion(args)
