from argparse import ArgumentParser, Namespace
from typing import Dict, Optional
from os import path
import subprocess
import os


EXIT_VERSION_NOT_FOUND = 20
EXIT_MAPPINGS_NOT_SUPPORTED = 21

SPECIAL_SOURCE_VERSION = "1.9.0"
SPECIAL_SOURCE_URL = f"https://repo1.maven.org/maven2/net/md-5/SpecialSource/{SPECIAL_SOURCE_VERSION}/SpecialSource-{SPECIAL_SOURCE_VERSION}-shaded.jar"

CFR_VERSION = "0.151"
CFR_URL = f"https://github.com/leibnitz27/cfr/releases/download/{CFR_VERSION}/cfr-{CFR_VERSION}.jar"

class CoderPackAddon:

    def __init__(self, pmc):

        self.pmc = pmc

        self.pmc.add_message("args.coderpack", "All subcommands for Minecraft developpers on latest versions like decompilation.")
        self.pmc.add_message("args.coderpack.decompile", "Remap, deobfuscate and decompile a version. Available since Minecaft 1.14.")
        self.pmc.add_message("args.coderpack.decompile.side", "Specify the game side to decompile, 'client' (default) or 'server'.")

        self.pmc.add_message("coderpack.decompile.not_supported", "This version is not supported for decompilation (jar or mappings unevailable).")
        self.pmc.add_message("coderpack.decompile.working_dir", "Working directory: {}")
        self.pmc.add_message("coderpack.decompile.converting_mappings", "Converting Proguard mappings to TSRG ones...")
        self.pmc.add_message("coderpack.decompile.converting_mappings_done", "=> Convertion done.")
        self.pmc.add_message("coderpack.decompile.remapping", "Remapping jar file using SpecialSource...")
        self.pmc.add_message("coderpack.decompile.decompiling.first_pass", "Decompiling using CFR, first pass...")
        self.pmc.add_message("coderpack.decompile.done", "Done")
        self.pmc.add_message("coderpack.decompile.nothing_done_suggest_delete", "The decompilation directory already exists, delete it to decompile: {}")
        self.pmc.add_message("coderpack.decompile.decompilation_done", "Decompilation done in this directory: {}")

        self.pmc.mixin("register_subcommands", self.register_subcommands)
        self.pmc.mixin("start_subcommand", self.start_subcommand)

    def register_subcommands(self, old, subcommands):
        # mixin
        old(subcommands)
        self.register_coderpack_arguments(subcommands.add_parser("coderpack", help=self.pmc.get_message("args.coderpack")))

    def start_subcommand(self, old, subcommand: str, args: Namespace) -> int:
        # mixin
        if subcommand == "coderpack":
            return self.cmd_coderpack(args)
        else:
            return old(subcommand, args)

    def register_coderpack_arguments(self, parser: ArgumentParser):
        self.register_coderpack_subcommands(parser.add_subparsers(title="coderpack subcommands", dest="coderpack_subcommand", required=True))

    def register_coderpack_subcommands(self, subcommands):
        self.register_decompile_arguments(subcommands.add_parser("decompile", help=self.pmc.get_message("args.coderpack.decompile")))

    def register_decompile_arguments(self, parser: ArgumentParser):
        parser.add_argument("-s", "--side", help=self.pmc.get_message("args.coderpack.decompile.side"), choices=["client", "server"], default="client")
        parser.add_argument("version")

    def cmd_coderpack(self, args: Namespace) -> int:
        if args.coderpack_subcommand == "decompile":
            return self.cmd_decompile(args)
        return 0

    def cmd_decompile(self, args: Namespace) -> int:

        VersionNotFoundError = self.pmc.VersionNotFoundError

        try:
            self.decompile(version=args.version, side=args.side)
        except VersionNotFoundError:
            return EXIT_VERSION_NOT_FOUND
        except MappingsNotSupportedError:
            return EXIT_MAPPINGS_NOT_SUPPORTED

    def decompile(self,
                  version: str,
                  side: str = "client",
                  out_dir: 'Optional[str]' = None) -> None:

        DownloadEntry = self.pmc.DownloadEntry

        self.pmc.check_main_dir()
        if out_dir is None:
            out_dir = path.join(self.pmc.get_main_dir(), "coderpack")

        # Resolve version metadata
        version, version_alias = self.pmc.get_version_manifest().filter_latest(version)
        version_meta, version_dir = self.pmc.resolve_version_meta_recursive(version)

        version_meta_downloads = version_meta["downloads"]
        proguard_download = version_meta_downloads.get("{}_mappings".format(side))
        jar_download = version_meta_downloads.get(side)

        if proguard_download is None or jar_download is None:
            self.pmc.print("coderpack.decompile.not_supported", side)
            raise MappingsNotSupportedError()

        # Actual version directory
        out_version_dir = path.join(out_dir, version)
        if not path.isdir(out_version_dir):
            os.makedirs(out_version_dir)
        self.pmc.print("coderpack.decompile.working_dir", out_version_dir)

        # Ensure proguard files
        proguard_file_name = "{}-{}.proguard".format(version, side)
        proguard_file = path.join(out_version_dir, proguard_file_name)
        if not path.isfile(proguard_file):
            self.pmc.download_file(DownloadEntry.from_version_meta_info(proguard_download, proguard_file, name=proguard_file_name))

        # Ensure TSRG file
        tsrg_file = path.join(out_version_dir, "{}-{}.tsrg".format(version, side))
        if not path.isfile(tsrg_file):
            self.pmc.print("coderpack.decompile.converting_mappings")
            tsrg_converter = ProguardToTsrgMapping(proguard_file, tsrg_file)
            tsrg_converter.convert()
            self.pmc.print("coderpack.decompile.converting_mappings_done")

        # Ensure version JAR file
        jar_file_name = "{}-{}.jar".format(version, side)
        jar_file = path.join(out_version_dir, jar_file_name)
        if not path.isfile(jar_file):
            self.pmc.download_file(DownloadEntry.from_version_meta_info(jar_download, jar_file, name=jar_file_name))

        bin_dir = path.join(out_dir, "bin")
        if not path.isdir(bin_dir):
            os.mkdir(bin_dir)

        # Ensure binaries
        def ensure_bin(url, file_name) -> str:
            source_file = path.join(bin_dir, file_name)
            if not path.isfile(source_file):
                self.pmc.download_file(DownloadEntry(url, source_file, name=file_name))
            return source_file

        special_source_file = ensure_bin(SPECIAL_SOURCE_URL, "SpecialSource-{}.jar".format(SPECIAL_SOURCE_VERSION))
        cfr_file = ensure_bin(CFR_URL, "cfr-{}.jar".format(CFR_VERSION))

        # Remapped JAR
        remapped_jar = path.join(out_version_dir, "{}-{}-remap.jar".format(version, side))
        if not path.isfile(remapped_jar):
            self.exec_jar(
                special_source_file,
                "--in-jar", jar_file,
                "--out-jar", remapped_jar,
                "--srg-in", tsrg_file,
                "--kill-lvt",
                title="coderpack.decompile.remapping")

        # Decompilation to an output directory
        decompiler_dir = path.join(out_version_dir, "{}-{}-out".format(version, side, ))
        if not path.isdir(decompiler_dir):
            os.mkdir(decompiler_dir)
            self.exec_jar(
                cfr_file,
                remapped_jar,
                "--outputdir", decompiler_dir,
                "--caseinsensitivefs", "true",
                title="coderpack.decompile.decompiling.first_pass")
            self.pmc.print("coderpack.decompile.decompilation_done", decompiler_dir)
        else:
            self.pmc.print("coderpack.decompile.nothing_done_suggest_delete", decompiler_dir)

    def exec_jar(self, pth, *args, title: str):
        self.pmc.print("", "===========================================")
        self.pmc.print(title)
        subprocess.run(["java", "-jar", pth, *args])
        self.pmc.print("coderpack.decompile.done")
        self.pmc.print("", "===========================================")


class MappingsNotSupportedError(Exception): ...


class ProguardToTsrgMapping:

    PRIMITIVES_REMAP = {
        "int": "I",
        "double": "D",
        "boolean": "Z",
        "float": "F",
        "long": "J",
        "byte": "B",
        "short": "S",
        "char": "C",
        "void": "V"
    }

    PATH_REMAP_JOINER = "/".join
    EMPTY_JOINER = "".join

    def __init__(self, src: str, dst: str):
        self.mappings: Dict[str, str] = {}
        self.src = src
        self.dst = dst

    @classmethod
    def _remap_path(cls, pth: str) -> str:
        return cls.PATH_REMAP_JOINER(pth.split("."))

    def _remap_type_no_array(self, typ: str) -> str:
        remap = self.PRIMITIVES_REMAP.get(typ)
        if remap is None:
            remap_path = self._remap_path(typ)
            remap = f"L{self.mappings.get(remap_path, remap_path)};"
        return remap

    def _remap_type(self, typ: str) -> str:
        try:
            idx = typ.index("[")
            dim = (len(typ) - idx) // 2
            remap = self._remap_type_no_array(typ[:idx])
            if dim != 0:
                remap = "{}{}".format("[" * dim, remap)
            return remap
        except ValueError:
            return self._remap_type_no_array(typ)

    def convert(self):
        self._analyze()
        self._process()

    def _analyze(self):
        self.mappings.clear()
        with open(self.src, "rt") as fp:
            for line in fp.readlines():
                if not line.startswith("#") and not line.startswith("    "):
                    deobf, obf = line.split(" -> ", maxsplit=2)
                    self.mappings[self._remap_path(deobf)] = self._remap_path(obf.rstrip("\r\n")[:-1])

    def _process(self):
        with open(self.src, "rt") as src_fp:
            with open(self.dst, "wt") as dst_fp:
                for line in src_fp.readlines():

                    if line.startswith("#"):
                        continue

                    deobf, obf = line.split(" -> ", maxsplit=2)

                    if deobf.startswith("   "):

                        deobf = deobf[4:]
                        typ, signature = deobf.split(" ", maxsplit=2)

                        dst_fp.write("\t")
                        dst_fp.write(obf.rstrip("\r\n"))

                        try:

                            # Delimitaters
                            signature_open = signature.index("(")
                            signature_close = signature.index(")")

                            # Split signature
                            args = signature[(signature_open + 1):signature_close]
                            signature = signature[:signature_open]

                            # The type is only kept for methods
                            try:
                                typ = typ[(typ.rindex(":") + 1):]
                            except ValueError:
                                pass

                            if len(args):
                                args = self.EMPTY_JOINER((self._remap_type(arg_type) for arg_type in args.split(",")))

                            dst_fp.write(" (")
                            dst_fp.write(args)
                            dst_fp.write(")")
                            dst_fp.write(self._remap_type(typ))

                        except ValueError:
                            pass

                        dst_fp.write(" ")
                        dst_fp.write(signature)

                    else:

                        dst_fp.write(self._remap_path(obf.rstrip("\r\n")[:-1]))
                        dst_fp.write(" ")
                        dst_fp.write(self._remap_path(deobf))

                    dst_fp.write("\n")
