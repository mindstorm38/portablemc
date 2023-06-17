"""Definition of the optimized download task.
"""

from http.client import HTTPConnection, HTTPSConnection, HTTPException
from threading import Thread
from pathlib import Path
from queue import Queue
import urllib.parse
import hashlib
import os

from .task import Task, State, Watcher

from typing import Optional, Dict, List, Tuple, Union


EV_DOWNLOAD_START = "download_start"
EV_DOWNLOAD_PROGRESS = "download_progress"
EV_DOWNLOAD_COMPLETE = "download_complete"


class DownloadTask(Task):
    """A download task.

    This task performs a mass download of files, this is basically
    used to download assets and libraries. This task setup a state
    that holds a `DownloadList` object. The state key can be specified
    and defaults to "dl".

    Input:
        <key> (Path): The download list 
    """

    def setup(self, state: State) -> None:
        state.insert(DownloadList())

    def execute(self, state: State, watcher: Watcher) -> None:

        dl = state[DownloadList]

        threads_count = (os.cpu_count() or 1) * 4
        threads: List[Thread] = []

        entries_queue = Queue()
        result_queue = Queue()

        for th_id in range(threads_count):
            th = Thread(target=download_thread, 
                        args=(th_id, entries_queue, result_queue), 
                        daemon=True, 
                        name=f"Download Thread {th_id}")
            th.start()
            threads.append(th)
        
        entries_count = len(dl.entries)
        result_count = 0

        watcher.on_event(EV_DOWNLOAD_START, threads_count=threads_count)

        for entry in dl.entries:
            entries_queue.put(entry)
        
        while result_count < entries_count:
            result = result_queue.get()
            if isinstance(result, _DownloadProgress):
                if result.complete:
                    result_count += 1
                watcher.on_event(EV_DOWNLOAD_PROGRESS, 
                    thread_id=result.thread_id,
                    entry=result.entry,
                    size=result.size,
                    count=result_count)
            else:
                result_count += 1

        # Send 'threads_count' sentinels.
        for th_id in range(threads_count):
            entries_queue.put(None)
        
        watcher.on_event(EV_DOWNLOAD_COMPLETE)

        # We intentionally don't join thread because it takes some time for unknown 
        # reason. And we don't care of these threads because these are daemon ones.


class DownloadEntry:
    """A download entry for the download task.
    """
    
    __slots__ = "url", "size", "sha1", "dst", "name"

    def __init__(self, 
        url: str, 
        dst: Path, *, 
        size: Optional[int] = None, 
        sha1: Optional[str] = None, 
        name: Optional[str] = None
    ) -> None:
        self.url = url
        self.dst = dst
        self.size = size
        self.sha1 = sha1
        self.name = url if name is None else name

    def __repr__(self) -> str:
        return f"<DownloadEntry {self.name}>"

    def __hash__(self) -> int:
        # Making size and sha1 in the hash is useful to make them, 
        # this means that once added to a dictionary, these attributes
        # should not be modified.
        return hash((self.url, self.dst, self.size, self.sha1))

    def __eq__(self, other):
        return (self.url, self.dst, self.size, self.sha1) == (other.url, other.dst, other.size, other.sha1)


class _DownloadEntry:
    """Internal class with already parsed URL.
    """

    __slots__ = "https", "host", "entry"

    def __init__(self, https: bool, host: str, entry: DownloadEntry) -> None:
        self.https = https
        self.host = host
        self.entry = entry
    
    @classmethod
    def from_entry(cls, entry: DownloadEntry) -> "_DownloadEntry":

        # We only support HTTP/HTTPS
        url_parsed = urllib.parse.urlparse(entry.url)
        if url_parsed.scheme not in ("http", "https"):
            raise ValueError(f"Illegal URL scheme '{url_parsed.scheme}://' for HTTP connection.")
        
        return cls(
            url_parsed.scheme == "https",
            url_parsed.netloc,
            entry)


class DownloadList:
    """ A download list.
    """

    __slots__ = "entries", "count", "size"

    def __init__(self):
        self.entries: List[_DownloadEntry] = []
        self.count = 0
        self.size = 0

    def add(self, entry: DownloadEntry, *, verify: bool = False):
        """Add a download entry to this list.

        :param entry: The entry to add.
        :param verify: Set to true in order to check if the file exists and has the same
        size has the given entry, in such case the entry is not added.
        """

        if verify and entry.dst.is_file() and (entry.size is None or entry.size == entry.dst.stat().st_size):
            return
        
        self.entries.append(_DownloadEntry.from_entry(entry))
        self.count += 1
        if entry.size is not None:
            self.size += entry.size


class _DownloadResult:

    __slots__ = "thread_id", "entry"

    def __init__(self, thread_id: int, entry: DownloadEntry) -> None:
        self.thread_id = thread_id
        self.entry = entry


class _DownloadProgress(_DownloadResult):
    
    __slots__ = "size", "complete"

    def __init__(self, thread_id: int, entry: DownloadEntry, size: int, complete: bool) -> None:
        super().__init__(thread_id, entry)
        self.size = size
        self.complete = complete


class _DownloadError(_DownloadResult):

    ERROR_CONN = "conn"
    ERROR_NOT_FOUND = "not_found"
    ERROR_INVALID_SIZE = "invalid_size"
    ERROR_INVALID_SHA1 = "invalid_sha1"

    __slots__ = "error",

    def __init__(self, thread_id: int, entry: DownloadEntry, error: str) -> None:
        super().__init__(thread_id, entry)
        self.error = error


def download_thread(
    thread_id: int, 
    entries_queue: Queue,
    result_queue: Queue,
):
    """This function is internally used for multi-threaded download.

    :param entries_queue: Where entries to download are received.
    :param result_queue: Where threads send progress update.
    """
    
    # Cache for connections depending on host and https
    conn_cache: Dict[Tuple[bool, str], Union[HTTPConnection, HTTPSConnection]] = {}

    # Each thread has its own buffer.
    buffer_back = bytearray(65536)
    buffer = memoryview(buffer_back)
    
    # Maximum tries count or a single entry.
    max_try_count = 3

    while True:

        raw_entry: Optional[_DownloadEntry] = entries_queue.get()

        # None is a sentinel to stop the thread, it should be consumed ONCE.
        if raw_entry is None:
            break

        conn_key = (raw_entry.https, raw_entry.host)
        conn = conn_cache.get(conn_key)

        if conn is None:
            conn_type = HTTPSConnection if raw_entry.https else HTTPConnection
            conn = conn_cache[conn_key] = conn_type(raw_entry.host)

        entry = raw_entry.entry
        
        # Allow modifying this URL when redirections happen.
        # size_target = 0 if entry.size is None else entry.size
        
        last_error: Optional[str] = None
        try_num = 0

        while True:

            try_num += 1
            if try_num > max_try_count:
                # Retrying implies that we have set an error.
                assert last_error is not None
                result_queue.put(_DownloadError(thread_id, entry, last_error))
            
            try:
                conn.request("GET", entry.url)
                res = conn.getresponse()
            except (ConnectionError, OSError, HTTPException):
                last_error = _DownloadError.ERROR_CONN
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

                last_error = _DownloadError.ERROR_NOT_FOUND
                continue

            sha1 = None if entry.sha1 is None else hashlib.sha1()
            size = 0

            entry.dst.parent.mkdir(parents=True, exist_ok=True)

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

                    result_queue.put(_DownloadProgress(
                        thread_id,
                        entry,
                        size,
                        False
                    ))

            if entry.size is not None and size != entry.size:
                last_error = _DownloadError.ERROR_INVALID_SIZE
            elif sha1 is not None and sha1.hexdigest() != entry.sha1:
                last_error = _DownloadError.ERROR_INVALID_SHA1
            else:
                
                result_queue.put(_DownloadProgress(
                    thread_id,
                    entry,
                    size,
                    True
                ))

                # Breaking means success.
                break

            # We are here only when the file download has started but checks have failed,
            # then we should remove the file.
            entry.dst.unlink(missing_ok=True)