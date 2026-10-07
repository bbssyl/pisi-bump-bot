import hashlib
import http.server
import os
import threading
import unittest
from unittest import mock

from pisi_bump_bot.archive_download import ArchiveDownloadError, download_sha1

PAYLOAD = b"pisi-bump-bot loopback payload\n" * 1000
LOOPBACK_ENVIRONMENT = {"no_proxy": "127.0.0.1,localhost", "NO_PROXY": "127.0.0.1,localhost"}


class PayloadHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        if self.path != "/archive.tar.gz":
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(PAYLOAD)))
        self.end_headers()
        self.wfile.write(PAYLOAD)

    def log_message(self, *_arguments: object) -> None:
        return None


class LoopbackDownloadTest(unittest.TestCase):
    def setUp(self) -> None:
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), PayloadHandler)
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        patcher = mock.patch.dict(os.environ, LOOPBACK_ENVIRONMENT)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.base_url = f"http://127.0.0.1:{self.server.server_address[1]}"

    def test_should_hash_archive_when_using_real_urlopen(self) -> None:
        result = download_sha1(f"{self.base_url}/archive.tar.gz")

        self.assertEqual((result.sha1, result.size_bytes), (hashlib.sha1(PAYLOAD).hexdigest(), len(PAYLOAD)))

    def test_should_raise_download_error_when_real_server_returns_404(self) -> None:
        with self.assertRaisesRegex(ArchiveDownloadError, "HTTP 404"):
            download_sha1(f"{self.base_url}/missing.tar.gz")

    def test_should_raise_download_error_when_port_is_closed(self) -> None:
        self.server.shutdown()
        self.server.server_close()

        with self.assertRaisesRegex(ArchiveDownloadError, "indirme başarısız"):
            download_sha1(f"{self.base_url}/archive.tar.gz")


if __name__ == "__main__":
    unittest.main()
