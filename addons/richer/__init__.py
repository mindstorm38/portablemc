
NAME = "Richer"
VERSION = "0.0.2"
AUTHORS = "Théo Rozier"
REQUIRES = "prompt_toolkit"
DESCRIPTION = "Better terminal for game process."


def addon_build(pmc):
    from .richer import RicherAddon
    return RicherAddon(pmc)
