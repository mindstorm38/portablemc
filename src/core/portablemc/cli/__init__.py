"""Main 
"""

import itertools
import time
import sys

from .util import format_locale_date, format_number, format_duration
from .output import Output, OutputTable, OutputTask
from .parse import register_arguments
from .lang import get as _

from ..download import DownloadStartEvent, DownloadProgressEvent, DownloadCompleteEvent, DownloadError
from ..standard import make_standard_sequence, Context, MetadataTask, JarTask, AssetsTask
from ..manifest import VersionManifest, VersionManifestError
from ..task import Watcher, Task

from typing import TYPE_CHECKING, Optional, List, Union, Dict, Callable, Any

if TYPE_CHECKING:
    from .parse import RootNs, SearchNs, StartNs


EXIT_OK = 0
EXIT_FAILURE = 1


def main(args: Optional[List[str]] = None):
    """Main entry point of the CLI. This function parses the input arguments and try to
    find a command handler to dispatch to. These command handlers are specified by the
    `get_command_handlers` function.
    """

    parser = register_arguments()
    ns = parser.parse_args(args or sys.argv[1:])

    command_handlers = get_command_handlers()
    command_attr = "subcommand"
    while True:
        command = getattr(ns, command_attr)
        handler = command_handlers.get(command)
        if handler is None:
            parser.print_help()
            sys.exit(EXIT_FAILURE)
        elif callable(handler):
            handler(ns)
        elif isinstance(handler, dict):
            command_attr = f"{command}_{command_attr}"
            command_handlers = handler
            continue
        sys.exit(EXIT_OK)


CommandFunc = Callable[[Any], Any]
CommandDict = Dict[str, Union[CommandFunc, "CommandDict"]]


def get_command_handlers() -> CommandDict:
    """This internal function returns the tree of command handlers for each subcommand
    of the CLI argument parser.
    """

    return {
        "search": cmd_search,
        "start": cmd_start,
        # "login": cmd_login,
        # "logout": cmd_logout,
        # "show": {
        #     "about": cmd_show_about,
        #     "auth": cmd_show_auth,
        #     "lang": cmd_show_lang,
        # },
        # "addon": {
        #     "list": cmd_addon_list,
        #     "show": cmd_addon_show
        # }
    }


def cmd_search(ns: "SearchNs"):

    def cmd_search_manifest(search: Optional[str], table: OutputTable):
        
        table.add(
            _("search.type"),
            _("search.name"),
            _("search.release_date"),
            _("search.flags"))
        table.separator()

        context = new_context(ns)
        manifest = new_version_manifest(ns)

        try:

            if search is not None:
                search, alias = manifest.filter_latest(search)
            else:
                alias = False

            for version_data in manifest.all_versions():
                version_id = version_data["id"]
                if search is None or (alias and search == version_id) or (not alias and search in version_id):
                    version = context.get_version(version_id)
                    table.add(
                        version_data["type"], 
                        version_id, 
                        format_locale_date(version_data["releaseTime"]),
                        _("search.flags.local") if version.metadata_exists() else "")

        except VersionManifestError as err:
            pass
            # print_task("FAILED", f"version_manifest.error.{err.code}", done=True)
            # sys.exit(EXIT_VERSION_NOT_FOUND)

    def cmd_search_local(search: Optional[str], table: OutputTable):

        table.add(
            _("search.name"),
            _("search.last_modified"))
        table.separator()

        raise NotImplementedError

    search_handler = {
        "manifest": cmd_search_manifest,
        "local": cmd_search_local
    }[ns.kind]

    table = ns.out.table()
    search_handler(ns.input, table)

    table.print()
    sys.exit(EXIT_OK)


def cmd_start(ns: "StartNs"):
    
    context = new_context(ns)
    manifest = new_version_manifest(ns)

    version_id, alias = manifest.filter_latest(ns.version)
        
    sequence = make_standard_sequence(version_id, context=context, version_manifest=manifest)
    sequence.add_watcher(StartWatcher(ns.out))
    sequence.add_watcher(DownloadWatcher(ns.out))

    sequence.execute()


def new_context(ns: "RootNs") -> Context:
    return Context(ns.main_dir, ns.work_dir)

def new_version_manifest(ns: "RootNs") -> VersionManifest:
    return VersionManifest()

    
class StartWatcher(Watcher):

    def __init__(self, out: Output) -> None:
        self.out = out
        self.out_task: Optional[OutputTask]
    
    def on_event(self, event: Any) -> None:
        pass


class DownloadWatcher(Watcher):
    """A watcher for pretty printing download task.
    """

    def __init__(self, out: Output) -> None:

        self.out = out
        self.out_task: OutputTask

        self.entries_count: int
        self.total_size: int
        self.size: int
        self.speeds: List[float]
    
    def on_event(self, event: Any) -> None:

        if isinstance(event, DownloadStartEvent):
            self.entries_count = event.entries_count
            self.total_size = event.size
            self.size = 0
            self.speeds = [0.0] * event.threads_count
            self.out_task = self.out.task()
            self.out_task.update("..", "download.start")

        elif isinstance(event, DownloadProgressEvent):
            self.speeds[event.thread_id] = event.speed
            speed = sum(self.speeds)
            self.size += event.size
            self.out_task.update("..", "download.progress", 
                speed=f"{format_number(speed)}o/s",
                count=event.count,
                total_count=self.entries_count,
                size=f"{format_number(self.size)}o")
            
        elif isinstance(event, DownloadCompleteEvent):
            self.out_task.update("OK", None)
            self.out_task.finish()

    def on_error(self, error: Exception) -> bool:

        if isinstance(error, DownloadError):
            self.out_task.update("FAILED", None)
            self.out_task.finish()
            for entry, code in error.errors:
                task = self.out.task()
                task.update(None, "download.error", name=entry.name, message=_(f"download.error.{code}"))
                task.finish()
            return True
        
        return False
