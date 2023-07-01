"""HTTP primitive functions.
"""

from http.client import HTTPConnection, HTTPSConnection, HTTPException
from http.client import HTTPResponse
from urllib.error import HTTPError
import urllib.request
import urllib.parse
import json
import ssl

from typing import Optional, Any, cast


__all__ = ["HttpResponse", "HttpError", "HttpSession"]


class HttpResponse:
    """An HTTP response containing the status, data and received headers.
    """
    
    def __init__(self, res: HTTPResponse) -> None:

        self.status = res.status
        self.data = res.read()
        self.headers = {}

        for header_name, header_value in res.getheaders():
            self.headers[header_name] = header_value

    def json(self) -> Any:
        """Parse the data as JSON. This may raise a JSONDecodeError.
        """
        return json.loads(self.data)

    def __repr__(self) -> str:
        return f"<HttpResponse {self.status}>"


class HttpError(Exception):
    """An HTTP error, raised when the status code of the response is not 200.
    """

    def __init__(self, res: HttpResponse, method: str, url: str) -> None:
        self.res = res
        self.method = method
        self.url = url

    def __repr__(self) -> str:
        return f"<HttpError {self.res}, origin: {self.method} {self.url}>"


class HttpSession:
    """Represent an HTTP session with practical methods for requesting files or resources
    from REST APIs for example.
    """

    def __init__(self, *, timeout: Optional[float] = None) -> None:
        self.timeout = timeout
    
    def request(self, method: str, url: str, *,
        data: Optional[bytes] = None,
        headers: Optional[dict] = None,
        accept: Optional[str] = None,
    ) -> HttpResponse:
        """Make an HTTP request.

        :return: The response returned should've a status of 2xx.
        :raises HttpError: An error wrapping a response that is not of status 2xx.
        """
        
        if headers is None:
            headers = {}
        
        if accept is not None:
            headers["Accept"] = accept
            
        # url_parsed = urllib.parse.urlparse(url)
        # if url_parsed.scheme not in ("http", "https"):
        #     raise ValueError(f"Illegal URL scheme '{url_parsed.scheme}://' for HTTP connection.")
        
        # if url_parsed.scheme == "https":

        #     try:
        #         import certifi
        #         ctx = ssl.create_default_context(cafile=certifi.where())
        #     except ImportError:
        #         ctx = None
            
        #     conn = HTTPSConnection(url_parsed.netloc, context=ctx)
        # else:
        #     conn = HTTPConnection(url_parsed.netloc)

        # conn.request(method, url, body=data, headers=headers)
        # res = conn.getresponse()

        # if res.status == 200:
        #     return HttpResponse(res)
        # else:
        #     raise HttpError(HttpResponse(res), method, url)

        try:
            import certifi
            ctx = ssl.create_default_context(cafile=certifi.where())
        except ImportError:
            ctx = None

        try:
            req = urllib.request.Request(url, data, headers, method=method)
            res: HTTPResponse = urllib.request.urlopen(req, timeout=self.timeout, context=ctx)
            return HttpResponse(res)
        except HTTPError as error:
            raise HttpError(HttpResponse(cast(HTTPResponse, error)), method, url)
    



# def http_request(url: str, method: str, *,
#     data: Optional[bytes] = None,
#     headers: Optional[dict] = None,
#     timeout: Optional[float] = None,
#     rcv_headers: Optional[dict] = None
# ) -> Tuple[int, bytes]:
#     """Make an HTTP request at a specified URL and retrieve raw data.
#     This is a simpler wrapper to the standard `url.request.urlopen` wrapper, it ignores 
#     HTTP errors and just return the error code with data.

#     :param url: The URL to request.
#     :param method: The HTTP method to use for this request.
#     :param data: Optional data to put in the request's body.
#     :param headers: Optional headers to add to default ones.
#     :param timeout: Optional timeout for the TCP handshake.
#     :param rcv_headers: Optional received headers dictionary.
#     :return: A tuple (HTTP response code, data bytes).
#     """

#     if headers is None:
#         headers = {}

#     try:

#         try:
#             import certifi
#             ctx = ssl.create_default_context(cafile=certifi.where())
#         except ImportError:
#             ctx = None

#         req = urllib.request.Request(url, data, headers, method=method)
#         res: HTTPResponse = urllib.request.urlopen(req, timeout=timeout, context=ctx)

#     except HTTPError as err:
#         # This type can be freely reinterpreted as HTTPResponse.
#         res = cast(HTTPResponse, err)

#     if rcv_headers is not None:
#         for header_name, header_value in res.getheaders():
#             rcv_headers[header_name] = header_value

#     return res.status, res.read() 


# def json_request(
#     url: str, method: str, *,
#     data: Optional[bytes] = None,
#     headers: Optional[dict] = None,
#     ignore_error: bool = False,
#     timeout: Optional[float] = None,
#     rcv_headers: Optional[dict] = None
# ) -> Tuple[int, dict]:
#     """A simple wrapper around ``http_request` function to decode 
#     returned data to JSON.

#     :param url: The URL to request.
#     :param method: The HTTP method to use for this request.
#     :param data: Optional data to put in the request's body.
#     :param headers: Optional headers to add to default ones.
#     :param ignore_error: Ignore JSON decodeing errors.
#     :param timeout: Optional timeout for the TCP handshake.
#     :param rcv_headers: Optional received headers dictionary.
#     :raises JSONDecodeError: If `ignore_error` is False and error.
#     :return: A tuple (HTTP response code, JSON dictionary).
#     """

#     if headers is None:
#         headers = {}
#     if "Accept" not in headers:
#         headers["Accept"] = "application/json"

#     status, data = http_request(url, method, data=data, headers=headers, timeout=timeout, rcv_headers=rcv_headers)

#     try:
#         # FIXME: Raise some error if the decoded value is not a dict.
#         return status, json.loads(data)
#     except JSONDecodeError:
#         if ignore_error:
#             return status, {"raw": data}
#         else:
#             raise


# def json_simple_request(url: str, *, ignore_error: bool = False, timeout: Optional[int] = None) -> dict:
#     """Make a GET request for a JSON API at specified URL. Might raise
#     `JsonRequestError` if failed.
#     """
#     return json_request(url, "GET", ignore_error=ignore_error, timeout=timeout)[1]
