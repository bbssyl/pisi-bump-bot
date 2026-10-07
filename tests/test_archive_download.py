import hashlib
import io
import unittest
import urllib.error
import urllib.request

from pisi_bump_bot.archive_download import ArchiveDownloadError, download_sha1


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


def opener_returning(response: FakeResponse):
    def opener(request: urllib.request.Request, timeout: float) -> FakeResponse:
        return response

    return opener


def opener_raising(error: BaseException):
    def opener(request: urllib.request.Request, timeout: float) -> FakeResponse:
        raise error

    return opener


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

    def test_should_accept_payload_equal_to_limit(self) -> None:
        result = download_sha1("https://example.org/a.zip", opener_returning(FakeResponse(b"q" * 10)), max_bytes=10)

        self.assertEqual(result.size_bytes, 10)


if __name__ == "__main__":
    unittest.main()
