"""HTTP primitive functions.
"""

from urllib.error import HTTPError, URLError
from http.client import HTTPResponse
import urllib.request
import urllib.parse
import json
import ssl

from . import LAUNCHER_VERSION

from typing import Optional, Any, cast


__all__ = ["HttpResponse", "HttpError", "http_request"]


class HttpResponse:
    """An HTTP response containing the status, data and received headers.
    """
    
    def __init__(self, res: Optional[HTTPResponse]) -> None:

        self.status = 0 if res is None else res.status
        self.data = b"null" if res is None else res.read()
        self.headers = {}

        if res is not None:
            for header_name, header_value in res.getheaders():
                self.headers[header_name] = header_value

    def json(self) -> Any:
        """Parse the data as JSON. This may raise a JSONDecodeError.
        """
        return json.loads(self.data)
    
    def text(self) -> str:
        """Parse the data as UTF-8 text.
        """
        return self.data.decode()

    def __repr__(self) -> str:
        return f"<HttpResponse {self.status}>"


class HttpError(Exception):
    """An HTTP error, raised when the status code of the response is not 200.

    If any network error happens and it's impossible to receive a response from the 
    server, an instance of `HttpResponse` with status equal to 0 is used (also has no 
    headers and `None` data). The original reason for this error is given in the `reason`
    attribute in any case.
    """

    def __init__(self, res: HttpResponse, method: str, url: str, reason: URLError) -> None:
        self.res = res
        self.method = method
        self.url = url
        self.reason = reason

    def __repr__(self) -> str:
        return f"<HttpError {self.res}, origin: {self.method} {self.url}, reason: {self.reason}>"


def http_request(method: str, url: str, *,
    data: Optional[bytes] = None,
    headers: Optional[dict] = None,
    accept: Optional[str] = None,
    content_type: Optional[str] = None
) -> HttpResponse:
    """Make a synchronous HTTP request.

    :return: The response returned should've a status of 2xx.
    :raises HttpError: An error wrapping a response that is not of status 2xx.
    """
    
    if headers is None:
        headers = {}
    if accept is not None:
        headers["Accept"] = accept
    if content_type is not None:
        headers["Content-Type"] = content_type
    if "User-Agent" not in headers:
        headers["User-Agent"] = f"portablemc/{LAUNCHER_VERSION}"

    try:
        import certifi
        ctx = ssl.create_default_context(cafile=certifi.where())
    except ImportError:
        ctx = None

    try:
        req = urllib.request.Request(url, data, headers, method=method)
        res: HTTPResponse = urllib.request.urlopen(req, context=ctx)
        return HttpResponse(res)
    except HTTPError as error:
        raise HttpError(HttpResponse(cast(HTTPResponse, error)), method, url, error)
    except URLError as error:
        raise HttpError(HttpResponse(None), method, url, error)
