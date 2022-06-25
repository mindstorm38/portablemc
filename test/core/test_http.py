import pytest

from portablemc import http_request, json_request, JsonRequestError
import socket


def test_http_request():

    code, data = http_request("https://httpbin.org/get", "GET")
    assert code == 200

    code, data = http_request("https://httpbin.org/post", "POST")
    assert code == 200

    code, data = http_request("https://httpbin.org/status/404", "GET")
    assert code == 404

    code, data = http_request("https://httpbin.org/status/404", "POST")
    assert code == 404

    rcv_headers = {}
    code, data = http_request("https://httpbin.org/response-headers?freeform=hello%20world!", "GET", rcv_headers=rcv_headers)
    assert code == 200 and rcv_headers["freeform"] == "hello world!"

    with pytest.raises((TimeoutError, socket.timeout)):
        http_request("https://httpbin.org/delay/2", "GET", timeout=1)


def test_json_request():

    code, data = json_request("https://httpbin.org/json", "GET")
    assert code == 200

    with pytest.raises(JsonRequestError) as err:
        json_request("https://httpbin.org/base64/aGVsbG8gd29ybGQh", "GET")

    assert err.value.code == JsonRequestError.INVALID_RESPONSE_NOT_JSON and\
        err.value.url == "https://httpbin.org/base64/aGVsbG8gd29ybGQh" and\
        err.value.method == "GET" and\
        err.value.status == 200 and\
        err.value.data == b"hello world!"

    code, data = json_request("https://httpbin.org/base64/aGVsbG8gd29ybGQh", "GET", ignore_error=True)
    assert code == 200 and data == {"raw": b"hello world!"}
