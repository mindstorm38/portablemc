
def get_pkg_version():
    try:
        import importlib.metadata
        return importlib.metadata.version("portablemc")
    except ImportError:
        import pkg_resources
        return pkg_resources.get_distribution("portablemc").version


def test_version():
    from portablemc import LAUNCHER_VERSION
    assert LAUNCHER_VERSION == get_pkg_version()
