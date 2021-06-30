
NAME = "FabricMC Manager"
VERSION = "0.0.2"
AUTHORS = "Théo Rozier"
REQUIRES = ()
DESCRIPTION = "FabricMC Manager, you can start Fabric using '<exec> start fabric:<mc-version>[:<loader-version>]'."


def addon_build(pmc):
    from .fabric import FabricAddon
    return FabricAddon(pmc)
