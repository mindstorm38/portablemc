
NAME = "Richer"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = "prompt_toolkit"

def addon_build(pmc):
    from .richer import RicherAddon
    return RicherAddon(pmc)
