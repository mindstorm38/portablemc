"""Definition of the optimized download task.
"""

from http.client import HTTPConnection, HTTPSConnection, HTTPException
from threading import Thread
from pathlib import Path
from queue import Queue
import urllib.parse
import hashlib
import time

from typing import Optional, Dict, List, Tuple, Union, Iterator


class DownloadEntry:
    """A download entry for the download task.
    """
    
    __slots__ = "url", "size", "sha1", "dst", "name", "executable"

    def __init__(self, 
        url: str, 
        dst: Path, *, 
        size: Optional[int] = None, 
        sha1: Optional[str] = None, 
        name: Optional[str] = None,
        executable: bool = False
    ) -> None:
        self.url = url
        self.dst = dst
        self.size = size
        self.sha1 = sha1
        self.name = url if name is None else name
        self.executable = executable

    def __repr__(self) -> str:
        return f"<DownloadEntry {self.name}>"

    def __hash__(self) -> int:
        # Making size and sha1 in the hash is useful to make them, 
        # this means that once added to a dictionary, these attributes
        # should not be modified.
        return hash((self.url, self.dst, self.size, self.sha1))

    def __eq__(self, other):
        return isinstance(other, DownloadEntry) and \
            (self.url, self.dst, self.size, self.sha1) == \
            (other.url, other.dst, other.size, other.sha1)


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
            raise ValueError(f"unsupported scheme '{url_parsed.scheme}://' from url {entry.url}")
        
        return cls(
            url_parsed.scheme == "https",
            url_parsed.netloc,
            entry)


class DownloadResult:
    """Base class for download result yielded by `DownloadList.download` function.
    """
    __slots__ = "thread_id", "entry"
    def __init__(self, thread_id: int, entry: DownloadEntry) -> None:
        self.thread_id = thread_id
        self.entry = entry


class DownloadResultProgress(DownloadResult):
    """Subclass of result when a file's download has been successful.
    """
    __slots__ = "size", "speed", "done"
    def __init__(self, thread_id: int, entry: DownloadEntry, size: int, speed: float, done: bool) -> None:
        super().__init__(thread_id, entry)
        self.size = size
        self.speed = speed
        self.done = done


class DownloadResultError(DownloadResult):
    """Subclass of result when a file's download has failed.
    """

    CONNECTION = "connection"
    NOT_FOUND = "not_found"
    INVALID_SIZE = "invalid_size"
    INVALID_SHA1 = "invalid_sha1"

    __slots__ = "code",

    def __init__(self, thread_id: int, entry: DownloadEntry, code: str) -> None:
        super().__init__(thread_id, entry)
        self.code = code


class DownloadList:
    """ A download list.
    """

    __slots__ = "entries", "count", "size"

    def __init__(self):
        self.entries: List[_DownloadEntry] = []
        self.count = 0
        self.size = 0
    
    def clear(self) -> None:
        """Clear the download entry, removing all entries and computed count/size.
        """
        self.entries.clear()
        self.count = 0
        self.size = 0

    def add(self, entry: DownloadEntry, *, verify: bool = False) -> None:
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
    
    def download(self, threads_count: int, *,
        partial_progress: bool = False
    ) -> Iterator[Tuple[int, DownloadResult]]:
        """Execute the download.
        
        :param threads_count: The number of threads to run the download on.
        :param partial_progress: Set to true to be able to receive partial progress update
        on unfinished files, if this is false, DownloadResultProgress.done should be true.
        :return: This function returns an iterator that yields a tuple that contain the
        total number of results and the new result that came in.
        """

        # Sort our entries in order to download big files first, this is allows better
        # parallelization at start and avoid too much blocking at the end of the download.
        # Note that entries without size are considered 1 Mio, to download early.
        self.entries.sort(key=lambda e: e.entry.size or 1048576, reverse=True)

        entries_count = len(self.entries)
        if not entries_count or threads_count < 1:
            return

        threads: List[Thread] = []

        entries_queue = Queue()
        result_queue = Queue()

        for th_id in range(threads_count):
            th = Thread(target=_download_thread, 
                        args=(th_id, entries_queue, result_queue, partial_progress), 
                        daemon=True, 
                        name=f"Download Thread {th_id}")
            th.start()
            threads.append(th)
        
        result_count = 0

        for entry in self.entries:
            entries_queue.put(entry)
        
        while result_count < entries_count:
            result = result_queue.get()
            if not isinstance(result, DownloadResultProgress) or result.done:
                result_count += 1
            yield result_count, result

        # Send 'threads_count' sentinels.
        for th_id in range(threads_count):
            entries_queue.put(None)

        # We intentionally don't join thread because it takes some time for unknown 
        # reason. And we don't care of these threads because these are daemon ones.
        pass


def _download_thread(
    thread_id: int, 
    entries_queue: Queue,
    result_queue: Queue,
    partial_progress: bool
):
    """This function is internally used for multi-threaded download.

    :param entries_queue: Where entries to download are received.
    :param result_queue: Where threads send progress update.
    """
    
    # Cache for connections depending on host and https
    conn_cache: Dict[Tuple[bool, str], Union[HTTPConnection, HTTPSConnection]] = {}

    # Each thread has its own buffer.
    buffer_cap = 65536
    buffer_back = bytearray(buffer_cap)
    buffer = memoryview(buffer_back)
    
    # Maximum tries count or a single entry.
    max_try_count = 3

    # For speed calculation.
    speed_update_interval = 0.25
    speed_smoothing = 0.3
    speed_last_time = 0
    speed_last_size = 0
    speed_current_size = 0
    speed = 0

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
                result_queue.put(DownloadResultError(thread_id, entry, last_error))
                break
            
            try:
                conn.request("GET", entry.url)
                res = conn.getresponse()
            except (ConnectionError, OSError, HTTPException):
                last_error = DownloadResultError.CONNECTION
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

                last_error = DownloadResultError.NOT_FOUND
                continue

            sha1 = None if entry.sha1 is None else hashlib.sha1()
            size = 0

            entry.dst.parent.mkdir(parents=True, exist_ok=True)
            with entry.dst.open("wb") as dst_fp:

                while True:

                    read_len = res.readinto(buffer)
                    if not read_len:
                        break

                    size += read_len
                    speed_current_size += read_len
                    buffer_view = buffer[:read_len]
                    if sha1 is not None:
                        sha1.update(buffer_view)
                    dst_fp.write(buffer_view)

                    # Update speed calculation at given interval.
                    now = time.monotonic()
                    speed_elapsed_time = now - speed_last_time
                    if speed_elapsed_time > speed_update_interval:
                        speed_elapsed_size = speed_current_size - speed_last_size
                        current_speed = speed_elapsed_size / speed_elapsed_time
                        speed = speed_smoothing * current_speed + (1 - speed_smoothing) * speed
                        speed_last_time = now
                        speed_last_size = speed_current_size

                    # Filled the whole buffer, send a progress update because we'll 
                    # likely need another reading.
                    if partial_progress and read_len == buffer_cap:
                        result_queue.put(DownloadResultProgress(
                            thread_id,
                            entry,
                            size,
                            speed,
                            False
                        ))
            
            # If the entry should be executable, only those that can read would be
            # able to execute it.
            if entry.executable:
                prev_mode = entry.dst.stat().st_mode
                entry.dst.chmod(prev_mode | ((prev_mode & 0o444) >> 2))

            if entry.size is not None and size != entry.size:
                last_error = DownloadResultError.INVALID_SIZE
            elif sha1 is not None and sha1.hexdigest() != entry.sha1:
                last_error = DownloadResultError.INVALID_SHA1
            else:
                
                result_queue.put(DownloadResultProgress(
                    thread_id,
                    entry,
                    size,
                    speed,
                    True
                ))

                # Breaking means success.
                break

            # We are here only when the file download has started but checks have failed,
            # then we should remove the file.
            try:
                entry.dst.unlink()
            except FileNotFoundError:
                pass  # Not a problem if the file isn't present.
