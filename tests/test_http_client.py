import http.client
import io
import socket
import ssl
import unittest
import urllib.error
from email.message import Message
from unittest import mock

from pisi_bump_bot.errors import FetchError
from pisi_bump_bot.github_upstream import GithubLookup
from pisi_bump_bot.http_client import stream_sha1, urllib_fetch
from pisi_bump_bot.package_checker import PackageChecker
from pisi_bump_bot.report_model import Status
from pisi_bump_bot.spec_parser import PackageRecipe

URL = "https://api.github.com/repos/o/r/releases/latest"
NETWORK_FAILURES = (
    urllib.error.URLError("unreachable"),
    socket.timeout("timed out"),
    ConnectionResetError("reset"),
    http.client.IncompleteRead(b"", 5),
    http.client.RemoteDisconnected("closed"),
    ssl.SSLError("handshake failed"),
)


class BrokenBody(io.BytesIO):
    def read(self, *_arguments: object) -> bytes:
        raise http.client.IncompleteRead(b"", 5)


def http_error(code: int, body: io.BytesIO) -> urllib.error.HTTPError:
    headers = Message()
    headers["X-RateLimit-Remaining"] = "0"
    return urllib.error.HTTPError(URL, code, "error", headers, body)


def raising_fetch(error: BaseException):
    def fetch(url, headers):
        raise error

    return fetch


class UrllibFetchTest(unittest.TestCase):
    def test_should_raise_fetch_error_for_every_network_failure(self) -> None:
        for failure in NETWORK_FAILURES:
            with self.subTest(failure=type(failure).__name__):
                with mock.patch("urllib.request.urlopen", side_effect=failure):
                    with self.assertRaises(FetchError):
                        urllib_fetch(URL, {})

    def test_should_return_status_and_lowercase_headers_when_http_error(self) -> None:
        with mock.patch("urllib.request.urlopen", side_effect=http_error(403, io.BytesIO(b"{}"))):
            response = urllib_fetch(URL, {})

        self.assertEqual((response.status, response.headers["x-ratelimit-remaining"]), (403, "0"))

    def test_should_raise_fetch_error_when_error_body_cannot_be_read(self) -> None:
        with mock.patch("urllib.request.urlopen", side_effect=http_error(502, BrokenBody())):
            with self.assertRaisesRegex(FetchError, "HTTP 502"):
                urllib_fetch(URL, {})

    def test_should_pass_timeout_as_keyword_when_calling_urlopen(self) -> None:
        with mock.patch("urllib.request.urlopen", side_effect=OSError("down")) as urlopen:
            with self.assertRaises(FetchError):
                urllib_fetch(URL, {})

        self.assertIn("timeout", urlopen.call_args.kwargs)

    def test_should_raise_fetch_error_when_hashing_fails(self) -> None:
        with mock.patch("urllib.request.urlopen", side_effect=http.client.BadStatusLine("x")):
            with self.assertRaises(FetchError):
                stream_sha1("https://github.com/o/r/archive/v1.tar.gz")


class LookupNetworkFailureTest(unittest.TestCase):
    def test_should_mark_package_as_error_and_continue_when_fetch_fails(self) -> None:
        recipe = PackageRecipe("r", "1.0", 1, ("https://github.com/o/r/archive/v1.0.tar.gz",), "a/r/pspec.xml")
        checker = PackageChecker(GithubLookup(raising_fetch(FetchError("ağ hatası")), None), None)

        report = checker.check(recipe)

        self.assertEqual((report.status, report.detail), (Status.ERROR, "ağ hatası"))


if __name__ == "__main__":
    unittest.main()
