import json
import re
from dataclasses import dataclass

from pisi_bump_bot.errors import FetchError, RateLimitExceeded, UpstreamError
from pisi_bump_bot.http_client import USER_AGENT, Fetch, HttpResponse
from pisi_bump_bot.version_compare import extract_version_text

API_ROOT = "https://api.github.com"
ARCHIVE_EXTENSION = r"(?:tar\.gz|tgz|zip|tar\.bz2|tar\.xz)"
OWNER_REPO = r"(?P<owner>[^/]+)/(?P<repo>[^/]+)"
ARCHIVE_PATTERNS = (
    re.compile(rf"^https?://github\.com/{OWNER_REPO}/releases/download/(?P<tag>[^/]+)/[^/]+$"),
    re.compile(rf"^https?://github\.com/{OWNER_REPO}/archive/refs/tags/(?P<tag>[^/]+?)\.{ARCHIVE_EXTENSION}$"),
    re.compile(rf"^https?://github\.com/{OWNER_REPO}/archive/(?P<tag>[^/]+?)\.{ARCHIVE_EXTENSION}$"),
    re.compile(rf"^https?://codeload\.github\.com/{OWNER_REPO}/(?:tar\.gz|zip)/(?:refs/tags/)?(?P<tag>[^/]+)$"),
    re.compile(rf"^https?://codeload\.github\.com/{OWNER_REPO}/(?:legacy\.)?(?:tar\.gz|zip)/(?:refs/tags/)?(?P<tag>[^/]+)$"),
)
MINIMUM_REPLACEABLE_VERSION_LENGTH = 3


@dataclass(frozen=True)
class GithubArchive:
    owner: str
    repo: str
    tag: str


@dataclass(frozen=True)
class LatestRelease:
    tag: str
    release_url: str


def parse_github_archive(url: str) -> GithubArchive | None:
    for pattern in ARCHIVE_PATTERNS:
        match = pattern.match(url)
        if match:
            return GithubArchive(match["owner"], match["repo"], match["tag"])
    return None


def build_candidate_url(url: str, old_tag: str, new_tag: str) -> str | None:
    if old_tag not in url:
        return None
    candidate = url.replace(old_tag, new_tag)
    old_text = extract_version_text(old_tag)
    new_text = extract_version_text(new_tag)
    if old_text and new_text and len(old_text) >= MINIMUM_REPLACEABLE_VERSION_LENGTH:
        candidate = candidate.replace(old_text, new_text)
    return candidate


def is_rate_limited(response: HttpResponse) -> bool:
    if response.status == 429:
        return True
    return response.status == 403 and response.headers.get("x-ratelimit-remaining") == "0"


def parse_json(response: HttpResponse) -> object:
    try:
        return json.loads(response.body)
    except ValueError as error:
        raise UpstreamError("GitHub yanıtı çözümlenemedi") from error


class GithubLookup:
    def __init__(self, fetch: Fetch, token: str | None) -> None:
        self._fetch = fetch
        self._token = token
        self._cache: dict[tuple[str, str], LatestRelease | UpstreamError] = {}
        self._rate_limited = False

    def latest(self, owner: str, repo: str) -> LatestRelease:
        key = (owner, repo)
        if key not in self._cache:
            self._cache[key] = self._resolve(owner, repo)
        cached = self._cache[key]
        if isinstance(cached, UpstreamError):
            raise cached
        return cached

    def _resolve(self, owner: str, repo: str) -> LatestRelease | UpstreamError:
        if self._rate_limited:
            raise RateLimitExceeded("rate limit")
        try:
            return self._latest_release(owner, repo)
        except UpstreamError as error:
            return error

    def _get(self, path: str) -> HttpResponse:
        headers = {"User-Agent": USER_AGENT, "Accept": "application/vnd.github+json"}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        try:
            response = self._fetch(f"{API_ROOT}{path}", headers)
        except FetchError as error:
            raise UpstreamError(str(error)) from error
        if is_rate_limited(response):
            self._rate_limited = True
            raise RateLimitExceeded("rate limit")
        return response

    def _latest_release(self, owner: str, repo: str) -> LatestRelease:
        response = self._get(f"/repos/{owner}/{repo}/releases/latest")
        if response.status == 404:
            return self._latest_tag(owner, repo)
        if response.status != 200:
            raise UpstreamError(f"GitHub HTTP {response.status}")
        payload = parse_json(response)
        if not isinstance(payload, dict) or payload.get("draft") or payload.get("prerelease"):
            return self._latest_tag(owner, repo)
        return self._release_from_tag(owner, repo, payload.get("tag_name"))

    def _latest_tag(self, owner: str, repo: str) -> LatestRelease:
        response = self._get(f"/repos/{owner}/{repo}/tags?per_page=1")
        if response.status == 404:
            raise UpstreamError("depo bulunamadı")
        if response.status != 200:
            raise UpstreamError(f"GitHub HTTP {response.status}")
        payload = parse_json(response)
        if not isinstance(payload, list) or not payload or not isinstance(payload[0], dict):
            raise UpstreamError("release veya tag yok")
        return self._release_from_tag(owner, repo, payload[0].get("name"))

    @staticmethod
    def _release_from_tag(owner: str, repo: str, tag: object) -> LatestRelease:
        if not isinstance(tag, str) or not tag:
            raise UpstreamError("tag adı okunamadı")
        return LatestRelease(tag, f"https://github.com/{owner}/{repo}/releases/tag/{tag}")
