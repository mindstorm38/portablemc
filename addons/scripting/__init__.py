
NAME = "Scripting"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "addon:richer", "prompt_toolkit"


def addon_build(pmc):
    from .scripting import ScriptingAddon
    return ScriptingAddon(pmc)
