"""Definition of the optimized download task.
"""

from http.client import HTTPConnection, HTTPSConnection, HTTPResponse, HTTPException
from threading import Thread
from pathlib import Path
from queue import Queue
import urllib.parse
import hashlib
import os

from .task import Task

from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from typing import Optional, Dict, List, Tuple, Union


class DownloadTask(Task):
    """A download task.

    This task performs a mass download of files, this is basically
    used to download assets and libraries. This task setup a state
    that holds a `DownloadList` object. The state key can be specified
    and defaults to "dl".

    Input:
        <key> (Path): The download list 
    """

    def __init__(self, key: str = "dl") -> None:
        self.key = key

    def setup(self, state: dict) -> None:
        state[self.key] = DownloadList()

    def execute(self, state: dict) -> None:

        dl: DownloadList = state[self.key]

        threads_count = os.cpu_count() or 1
        threads: List[Thread] = []

        entries_queue = Queue()
        result_queue = Queue()
        error_queue = Queue()

        for th_id in range(threads_count):
            th = Thread(target=download_thread, 
                        args=(th_id, entries_queue, result_queue, error_queue), 
                        daemon=True, 
                        name=f"Download thread {th_id}")
            th.start()
            threads.append(th)
        
        for entry in dl.entries:
            entries_queue.put(entry)

        for thread in threads:
            thread.join()


class DownloadList:
    """ A download list.
    """

    __slots__ = "entries", "count", "size"

    def __init__(self):
        self.entries: List[_DownloadEntry] = []
        self.count = 0
        self.size = 0

    def append(self, entry: "DownloadEntry"):
        self.entries.append(_DownloadEntry.from_entry(entry))
        self.count += 1
        if entry.size is not None:
            self.size += entry.size


class DownloadEntry:
    """A download entry for the download task.
    """
    
    __slots__ = "url", "size", "sha1", "dst", "name"

    def __init__(self, 
        url: str, 
        dst: "Path", *, 
        size: "Optional[int]" = None, 
        sha1: "Optional[str]" = None, 
        name: "Optional[str]" = None
    ) -> None:
        self.url = url
        self.dst = dst
        self.size = size
        self.sha1 = sha1
        self.name = url if name is None else name

    @classmethod
    def from_meta(cls, info: dict, dst: "Path", *, name: "Optional[str]" = None) -> 'DownloadEntry':
        if "url" not in info:
            raise ValueError("Missing required 'url' field in download meta.", info)
        return DownloadEntry(info["url"], dst, size=info.get("size"), sha1=info.get("sha1"), name=name)

    def __hash__(self) -> int:
        # Making size and sha1 in the hash is useful to make them, 
        # this means that once added to a dictionary, these attributes
        # should not be modified.
        return hash((self.url, self.dst, self.size, self.sha1))

    def __eq__(self, other):
        return (self.url, self.dst, self.size, self.sha1) == (other.url, other.dst, other.size, other.sha1)


class DownloadProgress:
    """A download progress.
    """
    
    __slots__ = "thread", "name", "size", "total"

    def __init__(self, thread: int, name: str, size: int, total: "Optional[int]") -> None:
        self.thread = thread
        self.name = name
        self.size = size
        self.total = total


class DownloadError:
    """A download error.
    """

    ERROR_CONN = "conn"
    ERROR_NOT_FOUND = "not_found"
    ERROR_INVALID_SIZE = "invalid_size"
    ERROR_INVALID_SHA1 = "invalid_sha1"

    __slots__ = "entry", "error"

    def __init__(self, entry: "DownloadEntry", error: str) -> None:
        self.entry = entry
        self.error = error


class _DownloadEntry:
    """Internal class with already parsed URL.
    """

    __slots__ = "https", "host", "entry"

    def __init__(self, https: bool, host: str, entry: "DownloadEntry") -> None:
        self.https = https
        self.host = host
        self.entry = entry
    
    @classmethod
    def from_entry(cls, entry: "DownloadEntry") -> "_DownloadEntry":

        # We only support HTTP/HTTPS
        url_parsed = urllib.parse.urlparse(entry.url)
        if url_parsed.scheme not in ("http", "https"):
            raise ValueError(f"Illegal URL scheme '{url_parsed.scheme}://' for HTTP connection.")
        
        return cls(
            url_parsed.scheme == "https",
            url_parsed.netloc,
            entry)


def download_thread(
    thread_id: int, 
    entries_queue: Queue,
    result_queue: Queue,
    error_queue: Queue,
):
    """This function is internally used for multi-threaded download.

    Args:
        entries_queue (Queue): Where entries to download are received.
        result_queue (Queue): Where threads send progress update.
    """
    
    # Cache for connections depending on host and https
    conn_cache: Dict[Tuple[bool, str], Union[HTTPConnection, HTTPSConnection]] = {}

    # Each thread has its own buffer.
    buffer_back = bytearray(65536)
    buffer = memoryview(buffer_back)
    
    # Maximum tries count or a single entry.
    max_try_count = 3

    while True:

        raw_entry: _DownloadEntry = entries_queue.get()

        conn_key = (raw_entry.https, raw_entry.host)
        conn = conn_cache.get(conn_key)

        if conn is None:
            conn_type = HTTPSConnection if raw_entry.https else HTTPConnection
            conn = conn_cache[conn_key] = conn_type(raw_entry.host)

        entry = raw_entry.entry
        
        # Allow modifying this URL when redirections happen.
        size_target = 0 if entry.size is None else entry.size
        last_error = None

        for try_num in range(max_try_count):

            try:
                conn.request("GET", entry.url)
                res = conn.getresponse()
            except (ConnectionError, OSError, HTTPException):
                last_error = DownloadError.ERROR_CONN
                continue

            if res.status == 301 or res.status == 302:

                redirect_url = res.headers["location"]
                redirect_entry = DownloadEntry(
                    redirect_url, 
                    entry.dst, 
                    size=entry.size, 
                    sha1=entry.sha1, 
                    name=entry.name)
                
                entries_queue.put(_DownloadEntry.from_entry(redirect_entry))
                break  # Abort on redirect

            elif res.status != 200:

                # This loop is used to skip all bytes in the stream, 
                # and allow further request.
                while res.readinto(buffer):
                    pass

                last_error = DownloadError.ERROR_NOT_FOUND
                continue

            sha1 = None if entry.sha1 is None else hashlib.sha1()
            size = 0

            entry.dst.parent.mkdir(parents=True, exist_ok=True)

            try:
                with entry.dst.open("wb") as dst_fp:
                    while True:
                        read_len = res.readinto(buffer)
                        if not read_len:
                            break
                        buffer_view = buffer[:read_len]
                        size += read_len
                        if sha1 is not None:
                            sha1.update(buffer_view)
                        dst_fp.write(buffer_view)
                        result_queue.put(DownloadProgress(
                            thread_id,
                            entry.name,
                            size,
                            entry.size,
                        ))
            except KeyboardInterrupt:
                entry.dst.unlink(missing_ok=True)
                raise

            if entry.size is not None and size != entry.size:
                last_error = DownloadError.ERROR_INVALID_SIZE
            elif sha1 is not None and sha1.hexdigest() != entry.sha1:
                last_error = DownloadError.ERROR_INVALID_SHA1
            else:
                # Break the for loop in order to skip the for-else branch
                break

            # We are here only when the file download has started but checks have failed,
            # then we should remove the file.
            entry.dst.unlink(missing_ok=True)

        else:
            # If the break was not triggered, an error should be set.
            assert last_error is not None
            error_queue.put(DownloadError(entry, last_error))
