import unittest

from pisi_bump_bot.errors import RateLimitExceeded, UpstreamError
from pisi_bump_bot.github_upstream import GithubArchive, GithubLookup, parse_github_archive
from tests.support import empty_response, json_response, routed_fetch

API = "https://api.github.com/repos/o/r"


class ParseGithubArchiveTest(unittest.TestCase):
    def test_should_parse_release_download_url(self) -> None:
        url = "https://github.com/atom/atom/releases/download/v1.57.0/atom-amd64.tar.gz"
        self.assertEqual(parse_github_archive(url), GithubArchive("atom", "atom", "v1.57.0"))

    def test_should_parse_refs_tags_archive_url(self) -> None:
        url = "https://github.com/ttcdt/mp-5.x/archive/refs/tags/5.55.tar.gz"
        self.assertEqual(parse_github_archive(url), GithubArchive("ttcdt", "mp-5.x", "5.55"))

    def test_should_parse_plain_archive_url_when_tag_contains_dots(self) -> None:
        url = "https://github.com/bulletphysics/bullet3/archive/3.08.tar.gz"
        self.assertEqual(parse_github_archive(url).tag, "3.08")

    def test_should_parse_codeload_url(self) -> None:
        url = "https://codeload.github.com/soimort/translate-shell/tar.gz/refs/tags/v0.9.6.12"
        self.assertEqual(parse_github_archive(url), GithubArchive("soimort", "translate-shell", "v0.9.6.12"))

    def test_should_return_none_when_archive_is_branch_snapshot(self) -> None:
        self.assertIsNone(parse_github_archive("https://github.com/a/b/archive/refs/heads/master.zip"))

    def test_should_return_none_when_host_is_not_github(self) -> None:
        self.assertIsNone(parse_github_archive("https://example.org/a/b/archive/1.0.tar.gz"))

    def test_should_return_none_when_url_has_typo_scheme(self) -> None:
        self.assertIsNone(parse_github_archive("hhttps://github.com/a/b/archive/1.0.tar.gz"))

    def test_should_return_none_when_url_is_raw_file(self) -> None:
        self.assertIsNone(parse_github_archive("https://github.com/groni/Sources/raw/master/x-1.0.tar.gz"))


class GithubLookupTest(unittest.TestCase):
    def test_should_return_latest_release_when_endpoint_succeeds(self) -> None:
        fetch = routed_fetch({f"{API}/releases/latest": json_response({"tag_name": "v2.0"})})
        result = GithubLookup(fetch, None).latest("o", "r")
        self.assertEqual((result.tag, result.release_url), ("v2.0", "https://github.com/o/r/releases/tag/v2.0"))

    def test_should_fall_back_to_tags_when_no_release_exists(self) -> None:
        fetch = routed_fetch({f"{API}/releases/latest": empty_response(404), f"{API}/tags": json_response([{"name": "t1"}])})
        self.assertEqual(GithubLookup(fetch, None).latest("o", "r").tag, "t1")

    def test_should_fall_back_to_tags_when_release_is_prerelease(self) -> None:
        fetch = routed_fetch(
            {f"{API}/releases/latest": json_response({"tag_name": "x", "prerelease": True}), f"{API}/tags": json_response([{"name": "t2"}])}
        )
        self.assertEqual(GithubLookup(fetch, None).latest("o", "r").tag, "t2")

    def test_should_raise_upstream_error_when_repository_missing(self) -> None:
        fetch = routed_fetch({})
        with self.assertRaises(UpstreamError):
            GithubLookup(fetch, None).latest("o", "r")

    def test_should_raise_rate_limit_when_quota_exhausted(self) -> None:
        fetch = routed_fetch({API: empty_response(403, {"x-ratelimit-remaining": "0"})})
        with self.assertRaises(RateLimitExceeded):
            GithubLookup(fetch, None).latest("o", "r")

    def test_should_not_call_api_again_when_rate_limited(self) -> None:
        calls: list[str] = []
        lookup = GithubLookup(routed_fetch({API: empty_response(429)}, calls), None)
        for repo in ("r", "other"):
            with self.assertRaises(RateLimitExceeded):
                lookup.latest("o", repo)
        self.assertEqual(len(calls), 1)

    def test_should_request_once_when_same_repository_is_asked_twice(self) -> None:
        calls: list[str] = []
        lookup = GithubLookup(routed_fetch({f"{API}/releases/latest": json_response({"tag_name": "v1"})}, calls), None)
        lookup.latest("o", "r")
        lookup.latest("o", "r")
        self.assertEqual(len(calls), 1)

    def test_should_send_authorization_header_when_token_given(self) -> None:
        seen: dict[str, str] = {}

        def fetch(url, headers):
            seen.update(headers)
            return json_response({"tag_name": "v1"})

        GithubLookup(fetch, "secret").latest("o", "r")
        self.assertEqual(seen["Authorization"], "Bearer secret")

    def test_should_raise_upstream_error_when_json_is_invalid(self) -> None:
        fetch = routed_fetch({f"{API}/releases/latest": empty_response(200)})
        with self.assertRaises(UpstreamError):
            GithubLookup(fetch, None).latest("o", "r")
