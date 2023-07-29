import pytest


def pytest_addoption(parser):
    parser.addoption("--runslow", action="store_true", default=False, help="run slow tests")

def pytest_configure(config):
    config.addinivalue_line("markers", "slow: mark test as slow to run")

def pytest_collection_modifyitems(config, items):

    if config.getoption("--runslow"):
        return
    
    skip_slow = pytest.mark.skip(reason="need --runslow option to run")
    for item in items:
        if "slow" in item.keywords:
            item.add_marker(skip_slow)

def pytest_generate_tests(metafunc):
    if "vanilla_version" in metafunc.fixturenames:
        from portablemc.standard import VersionManifest
        manifest = VersionManifest()
        metafunc.parametrize("vanilla_version", map(lambda v: v["id"], filter(lambda v: v["type"] in ("release", "old_beta", "old_alpha"), manifest.all_versions())))

@pytest.fixture(scope = "session")
def tmp_context(tmp_path_factory):
    """This fixture is used to create a game's install context global to test session.
    """

    from portablemc.standard import Context
    return Context(tmp_path_factory.mktemp("context"))
