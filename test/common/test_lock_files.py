

def test_lock_files():

    # This test check is all add-on have the right development version of "core" locked in "poetry.lock".
    # There has been too much issues with this in the past.

    from pathlib import Path
    import portablemc
    import toml

    src_dir = Path(__file__).parent.parent.parent / "src"

    core_dir = src_dir / "core"
    core_pyproject_file = core_dir / "pyproject.toml"
    core_pyproject_data = toml.load(core_pyproject_file)

    core_version = core_pyproject_data["tool"]["poetry"]["version"]
    assert core_version == portablemc.LAUNCHER_VERSION, "incoherent core's version"

    for module_dir in src_dir.iterdir():
        if module_dir.is_dir() and module_dir.name != "core":
            lock_file = module_dir / "poetry.lock"
            lock_data = toml.load(lock_file)
            for pkg_data in lock_data["package"]:
                if pkg_data["name"] == "portablemc":
                    assert pkg_data["version"] == core_version, f"module is not locked to the lastest core's version: {module_dir.name}"
                    assert pkg_data["source"]["type"] == "directory"
                    assert pkg_data["source"]["url"] == "../core"
