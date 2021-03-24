
NAME = "Scripting"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "addon:richer", "prompt_toolkit"
DESCRIPTION = "Improve the 'richer' addon's game terminal by adding a Python interpreter for the java's reflection API."


def addon_build(pmc):
    from .scripting import ScriptingAddon
    return ScriptingAddon(pmc)
