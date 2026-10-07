import hashlib
import http.client
import urllib.error
import urllib.request
from collections.abc import Callable, Mapping
from dataclasses import dataclass

from pisi_bump_bot.errors import FetchError

USER_AGENT = "pisi-bump-bot"
REQUEST_TIMEOUT_SECONDS = 60
HASH_CHUNK_BYTES = 1024 * 1024
NETWORK_ERRORS = (urllib.error.URLError, http.client.HTTPException, OSError)


@dataclass(frozen=True)
class HttpResponse:
    status: int
    headers: Mapping[str, str]
    body: bytes


Fetch = Callable[[str, Mapping[str, str]], HttpResponse]
HashArchive = Callable[[str], str]


def lowercase_headers(headers: Mapping[str, str]) -> dict[str, str]:
    return {name.lower(): value for name, value in headers.items()}


def urllib_fetch(url: str, headers: Mapping[str, str]) -> HttpResponse:
    request = urllib.request.Request(url, headers=dict(headers))
    try:
        with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            return HttpResponse(response.status, lowercase_headers(response.headers), response.read())
    except urllib.error.HTTPError as error:
        return read_error_response(url, error)
    except NETWORK_ERRORS as error:
        raise FetchError(f"{url}: {type(error).__name__}: {error}") from error


def read_error_response(url: str, error: urllib.error.HTTPError) -> HttpResponse:
    try:
        return HttpResponse(error.code, lowercase_headers(error.headers or {}), error.read())
    except NETWORK_ERRORS as read_error:
        raise FetchError(f"{url}: HTTP {error.code} yanıtı okunamadı") from read_error


def stream_sha1(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    digest = hashlib.sha1()
    try:
        with urllib.request.urlopen(request, timeout=REQUEST_TIMEOUT_SECONDS) as response:
            for chunk in iter(lambda: response.read(HASH_CHUNK_BYTES), b""):
                digest.update(chunk)
    except NETWORK_ERRORS as error:
        raise FetchError(f"{url}: {type(error).__name__}: {error}") from error
    return digest.hexdigest()
