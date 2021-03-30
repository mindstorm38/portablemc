
NAME = "Coder Pack"
VERSION = "0.0.1"
AUTHORS = "Théo Rozier"
REQUIRES = ()
DESCRIPTION = "Minecraft coder utilities, JAR remapping, deobfuscation and decompilation for latest versions."


def addon_build(pmc):
    from .coderpack import CoderPackAddon
    return CoderPackAddon(pmc)
