import hashlib
import urllib.error
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from typing import Any

from pisi_bump_bot.errors import BotError

USER_AGENT = "pisi-bump-bot"
DEFAULT_MAX_BYTES = 2 * 1024 * 1024 * 1024
DOWNLOAD_TIMEOUT_SECONDS = 60
CHUNK_BYTES = 1024 * 1024

Opener = Callable[[urllib.request.Request, float], Any]


class ArchiveDownloadError(BotError):
    pass


@dataclass(frozen=True)
class DownloadResult:
    url: str
    sha1: str
    size_bytes: int


def declared_length(response: Any) -> int | None:
    value = response.headers.get("Content-Length") if getattr(response, "headers", None) else None
    return int(value) if value and value.isdigit() else None


def reject_oversize(length: int | None, max_bytes: int) -> None:
    if length is not None and length > max_bytes:
        raise ArchiveDownloadError(f"arşiv çok büyük ({length} bayt, sınır {max_bytes} bayt)")


def hash_stream(response: Any, max_bytes: int) -> tuple[str, int]:
    digest = hashlib.sha1()
    size = 0
    for chunk in iter(lambda: response.read(CHUNK_BYTES), b""):
        size += len(chunk)
        reject_oversize(size, max_bytes)
        digest.update(chunk)
    return digest.hexdigest(), size


def download_sha1(
    url: str, opener: Opener = urllib.request.urlopen, max_bytes: int = DEFAULT_MAX_BYTES
) -> DownloadResult:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with opener(request, DOWNLOAD_TIMEOUT_SECONDS) as response:
            reject_oversize(declared_length(response), max_bytes)
            sha1, size = hash_stream(response, max_bytes)
    except urllib.error.HTTPError as error:
        raise ArchiveDownloadError(f"indirme başarısız (HTTP {error.code})") from error
    except TimeoutError as error:
        raise ArchiveDownloadError("indirme zaman aşımına uğradı") from error
    except (urllib.error.URLError, OSError) as error:
        raise ArchiveDownloadError(f"indirme başarısız ({error})") from error
    return DownloadResult(url, sha1, size)
