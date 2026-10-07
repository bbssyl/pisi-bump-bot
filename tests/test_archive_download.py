import hashlib
import http.client
import io
import socket
import ssl
import unittest
import urllib.error
import urllib.request

from pisi_bump_bot.archive_download import DOWNLOAD_TIMEOUT_SECONDS, ArchiveDownloadError, download_sha1

DEFAULT_TIMEOUT = getattr(socket, "_GLOBAL_DEFAULT_TIMEOUT", object())


class FakeResponse:
    def __init__(self, payload: bytes, content_length: str | None = None) -> None:
        self.headers = {"Content-Length": content_length} if content_length else {}
        self.reads: list[int] = []
        self.payload = payload
        self.offset = 0

    def read(self, size: int) -> bytes:
        self.reads.append(size)
        chunk = self.payload[self.offset : self.offset + size]
        self.offset += len(chunk)
        return chunk

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *_arguments: object) -> None:
        return None


class UrlopenLike:
    def __init__(self, response: FakeResponse | None = None, error: BaseException | None = None) -> None:
        self.response = response
        self.error = error
        self.calls: list[tuple[object, object, object]] = []

    def __call__(self, url: object, data: object = None, timeout: object = DEFAULT_TIMEOUT) -> FakeResponse:
        self.calls.append((url, data, timeout))
        if data is not None:
            raise TypeError("message_body should be a bytes-like object or an iterable")
        if self.error is not None:
            raise self.error
        return self.response


def opener_returning(response: FakeResponse) -> UrlopenLike:
    return UrlopenLike(response=response)


def opener_raising(error: BaseException) -> UrlopenLike:
    return UrlopenLike(error=error)


class DownloadSha1Test(unittest.TestCase):
    def test_should_return_sha1_and_size_when_download_succeeds(self) -> None:
        payload = b"x" * (2 * 1024 * 1024 + 5)

        result = download_sha1("https://example.org/a.zip", opener_returning(FakeResponse(payload)))

        self.assertEqual(result.sha1, hashlib.sha1(payload).hexdigest())
        self.assertEqual(result.size_bytes, len(payload))

    def test_should_read_in_chunks_when_payload_is_large(self) -> None:
        response = FakeResponse(b"y" * (3 * 1024 * 1024))

        download_sha1("https://example.org/a.zip", opener_returning(response))

        self.assertTrue(all(size <= 1024 * 1024 for size in response.reads))

    def test_should_raise_turkish_error_when_http_error(self) -> None:
        error = urllib.error.HTTPError("https://example.org/a.zip", 404, "Not Found", {}, io.BytesIO(b""))

        with self.assertRaisesRegex(ArchiveDownloadError, "HTTP 404"):
            download_sha1("https://example.org/a.zip", opener_raising(error))

    def test_should_raise_when_content_length_exceeds_limit(self) -> None:
        response = FakeResponse(b"abc", content_length="100")

        with self.assertRaisesRegex(ArchiveDownloadError, "çok büyük"):
            download_sha1("https://example.org/a.zip", opener_returning(response), max_bytes=10)

    def test_should_raise_when_stream_exceeds_limit_without_length_header(self) -> None:
        response = FakeResponse(b"z" * 50)

        with self.assertRaisesRegex(ArchiveDownloadError, "çok büyük"):
            download_sha1("https://example.org/a.zip", opener_returning(response), max_bytes=10)

    def test_should_raise_timeout_reason_when_connection_times_out(self) -> None:
        with self.assertRaisesRegex(ArchiveDownloadError, "zaman aşımı"):
            download_sha1("https://example.org/a.zip", opener_raising(TimeoutError()))

    def test_should_raise_when_network_unreachable(self) -> None:
        error = urllib.error.URLError("unreachable")

        with self.assertRaisesRegex(ArchiveDownloadError, "indirme başarısız"):
            download_sha1("https://example.org/a.zip", opener_raising(error))

    def test_should_pass_timeout_as_keyword_and_no_body_when_calling_urlopen(self) -> None:
        opener = opener_returning(FakeResponse(b"abc"))

        download_sha1("https://example.org/a.zip", opener)

        request, data, timeout = opener.calls[0]
        self.assertEqual((request.full_url, data, timeout), ("https://example.org/a.zip", None, DOWNLOAD_TIMEOUT_SECONDS))

    def test_should_send_get_request_with_user_agent(self) -> None:
        opener = opener_returning(FakeResponse(b"abc"))

        download_sha1("https://example.org/a.zip", opener)

        request = opener.calls[0][0]
        self.assertEqual((request.get_method(), request.get_header("User-agent")), ("GET", "pisi-bump-bot"))

    def test_should_convert_every_network_failure_to_download_error(self) -> None:
        failures = (
            urllib.error.URLError(TimeoutError("timed out")),
            socket.timeout("timed out"),
            ConnectionResetError("reset"),
            ConnectionRefusedError("refused"),
            OSError("network down"),
            http.client.IncompleteRead(b"partial", 10),
            http.client.RemoteDisconnected("closed"),
            http.client.BadStatusLine("garbage"),
            http.client.InvalidURL("bad url"),
            ssl.SSLError("handshake failed"),
        )
        for failure in failures:
            with self.subTest(failure=type(failure).__name__):
                with self.assertRaisesRegex(ArchiveDownloadError, "indirme"):
                    download_sha1("https://example.org/a.zip", opener_raising(failure))

    def test_should_convert_failure_raised_while_reading_body(self) -> None:
        response = FakeResponse(b"")
        response.read = lambda size: (_ for _ in ()).throw(http.client.IncompleteRead(b"", 5))

        with self.assertRaisesRegex(ArchiveDownloadError, "IncompleteRead"):
            download_sha1("https://example.org/a.zip", opener_returning(response))

    def test_should_report_timeout_when_url_error_wraps_timeout(self) -> None:
        with self.assertRaisesRegex(ArchiveDownloadError, "zaman aşımı"):
            download_sha1("https://example.org/a.zip", opener_raising(urllib.error.URLError(TimeoutError())))

    def test_should_let_programming_errors_surface(self) -> None:
        with self.assertRaises(TypeError):
            download_sha1("https://example.org/a.zip", opener_raising(TypeError("bug")))

    def test_should_accept_payload_equal_to_limit(self) -> None:
        result = download_sha1("https://example.org/a.zip", opener_returning(FakeResponse(b"q" * 10)), max_bytes=10)

        self.assertEqual(result.size_bytes, 10)


if __name__ == "__main__":
    unittest.main()
