
NAME = "Richer"
VERSION = "0.0.2"
AUTHORS = "Théo Rozier"
REQUIRES = "prompt_toolkit"
DESCRIPTION = "Improve downloads progress bars and the game process terminal."


def addon_build(pmc):
    from .richer import RicherAddon
    return RicherAddon(pmc)
