
NAME = "FabricMC Manager"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = ()
DESCRIPTION = "Fabric mod loader manager, this add-on handles start using 'fabric:<mc-version>[:<loader-version>]'."


def addon_build(pmc):
    from .fabric import FabricAddon
    return FabricAddon(pmc)
