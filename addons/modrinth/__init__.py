from argparse import ArgumentParser, Namespace
import sys


def load(pmc):

    @pmc.mixin()
    def register_subcommands(old, subparsers):
        _ = pmc.get_message
        old(subparsers)
        register_mod_arguments(subparsers.add_parser("mod", help=_("args.mod")))

    def register_mod_arguments(parser: ArgumentParser):
        _ = pmc.get_message
        subparsers = parser.add_subparsers(title="subcommands", dest="mod_subcommand")
        subparsers.required = True
        register_mod_search_arguments(subparsers.add_parser("search", help=_("args.mod.search")))
        register_mod_install_arguments(subparsers.add_parser("install", help=_("args.mod.about")))

    def register_mod_search_arguments(parser: ArgumentParser):
        parser.add_argument("query", nargs="?")

    def register_mod_install_arguments(parser: ArgumentParser):
        parser.add_argument("specifier")
        # TODO: Specifier must be of the form <mod_id>[:<game_version_id>[-<forge|fabric>]][@<mod_version_id>]
        # The following are equivalent
        # sodium
        # sodium:1.17.1
        # sodium:1.17.1-fabric
        # sodium:1.17.1@mc1.17.1-0.3.2
        # sodium:1.17.1-fabric@mc1.17.1-0.3.2
        #
        # TODO Another syntax:
        # sodium@1.17.1
        # sodium@1.17.1-fabric
        # sodium/mc1.17.1-0.3.2
        #
        # sodium/mc1.16.3-0.1.0
        # sodium/mc1.16.3-0.1.0@1.16.5
        # sodium/mc1.16.3-0.1.0@1.16.3
        # sodium/mc1.16.3-0.1.0@1.16.5-fabric
        #
        # File structure will be like this:
        # + mods
        #   + fabric-1.17.1
        #     + sodium-fabric-mc1.17.1-0.3.2.jar
        #   + fabric-1.16.5
        #     + sodium-fabric-mc1.16.3-0.1.0.jar
        #   + fabric-1.16.3
        #     + sodium-fabric-mc1.16.3-0.1.0.jar
        #

    # Messages

    pmc.messages.update({
        "args.mod": "Mods manager using modrinth.",
        "args.mod.search": "Search for mods.",
        "args.mod.install": "Install mods."
    })
