"""HTTP primitive functions.
"""

from http.client import HTTPResponse
from urllib.error import HTTPError
from json import JSONDecodeError
import urllib.request
import json
import ssl

from typing import Optional, Tuple, cast


def http_request(url: str, method: str, *,
    data: Optional[bytes] = None,
    headers: Optional[dict] = None,
    timeout: Optional[float] = None,
    rcv_headers: Optional[dict] = None
) -> Tuple[int, bytes]:
    """Make an HTTP request at a specified URL and retrieve raw data.
    This is a simpler wrapper to the standard `url.request.urlopen` wrapper, it ignores 
    HTTP errors and just return the error code with data.

    :param url: The URL to request.
    :param method: The HTTP method to use for this request.
    :param data: Optional data to put in the request's body.
    :param headers: Optional headers to add to default ones.
    :param timeout: Optional timeout for the TCP handshake.
    :param rcv_headers: Optional received headers dictionary.
    :return: A tuple (HTTP response code, data bytes).
    """

    if headers is None:
        headers = {}

    try:

        try:
            import certifi
            ctx = ssl.create_default_context(cafile=certifi.where())
        except ImportError:
            ctx = None

        req = urllib.request.Request(url, data, headers, method=method)
        res: HTTPResponse = urllib.request.urlopen(req, timeout=timeout, context=ctx)

    except HTTPError as err:
        # This type can be freely reinterpreted as HTTPResponse.
        res = cast(HTTPResponse, err)

    if rcv_headers is not None:
        for header_name, header_value in res.getheaders():
            rcv_headers[header_name] = header_value

    return res.status, res.read()


def json_request(
    url: str, method: str, *,
    data: Optional[bytes] = None,
    headers: Optional[dict] = None,
    ignore_error: bool = False,
    timeout: Optional[float] = None,
    rcv_headers: Optional[dict] = None
) -> Tuple[int, dict]:
    """A simple wrapper around ``http_request` function to decode 
    returned data to JSON. If decoding fails and parameter 
    `ignore_error` is false, error `JsonRequestError` is raised with 
    `JsonRequestError.INVALID_RESPONSE_NOT_JSON`.

    :param url: The URL to request.
    :param method: The HTTP method to use for this request.
    :param data: Optional data to put in the request's body.
    :param headers: Optional headers to add to default ones.
    :param ignore_error: Ignore JSON decodeing errors.
    :param timeout: Optional timeout for the TCP handshake.
    :param rcv_headers: Optional received headers dictionary.
    :raises JSONDecodeError: If `ignore_error` is False and error.
    :return: _description_
    """

    if headers is None:
        headers = {}
    if "Accept" not in headers:
        headers["Accept"] = "application/json"

    status, data = http_request(url, method, data=data, headers=headers, timeout=timeout, rcv_headers=rcv_headers)

    try:
        return status, json.loads(data)
    except JSONDecodeError:
        if ignore_error:
            return status, {"raw": data}
        else:
            raise


def json_simple_request(url: str, *, ignore_error: bool = False, timeout: Optional[int] = None) -> dict:
    """Make a GET request for a JSON API at specified URL. Might raise
    `JsonRequestError` if failed.
    """
    return json_request(url, "GET", ignore_error=ignore_error, timeout=timeout)[1]
