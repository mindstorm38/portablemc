
def test_project_version():

    from portablemc import LAUNCHER_VERSION
    from pathlib import Path
    import toml

    pyproject_file = Path(__file__).parent.parent / "pyproject.toml"
    pyproject = toml.load(pyproject_file)

    assert pyproject["tool"]["poetry"]["version"] == LAUNCHER_VERSION, "incoherent version"
