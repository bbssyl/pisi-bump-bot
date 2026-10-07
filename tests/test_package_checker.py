import unittest

from pisi_bump_bot.errors import FetchError
from pisi_bump_bot.github_upstream import GithubLookup
from pisi_bump_bot.spec_parser import PackageRecipe
from pisi_bump_bot.package_checker import PackageChecker
from pisi_bump_bot.report_model import Status
from tests.support import empty_response, json_response, routed_fetch

RELEASE_URL = "https://github.com/o/r/archive/refs/tags/v1.0.tar.gz"


def recipe(version: str = "1.0", url: str = RELEASE_URL) -> PackageRecipe:
    return PackageRecipe("pkg", version, 1, (url,), "pkg/pspec.xml")


def checker(tag: str | None, hash_archive=None) -> PackageChecker:
    routes = {} if tag is None else {"https://api.github.com/repos/o/r/releases/latest": json_response({"tag_name": tag})}
    return PackageChecker(GithubLookup(routed_fetch(routes), None), hash_archive)


class PackageCheckerTest(unittest.TestCase):
    def test_should_report_outdated_with_candidate_when_newer_release_exists(self) -> None:
        report = checker("v1.1").check(recipe())
        self.assertEqual(
            (report.status, report.candidate_url),
            (Status.OUTDATED, "https://github.com/o/r/archive/refs/tags/v1.1.tar.gz"),
        )

    def test_should_report_current_when_versions_match_after_normalization(self) -> None:
        self.assertEqual(checker("v1.0").check(recipe()).status, Status.CURRENT)

    def test_should_report_unsupported_when_archive_is_not_github(self) -> None:
        report = checker("v2").check(recipe(url="https://example.org/a.tar.gz"))
        self.assertEqual(report.status, Status.UNSUPPORTED)

    def test_should_report_unsupported_when_no_archive_exists(self) -> None:
        report = checker("v2").check(PackageRecipe("pkg", "1", 1, (), ""))
        self.assertEqual(report.status, Status.UNSUPPORTED)

    def test_should_report_uncomparable_when_tag_has_no_version(self) -> None:
        self.assertEqual(checker("latest").check(recipe()).status, Status.UNCOMPARABLE)

    def test_should_report_error_when_repository_lookup_fails(self) -> None:
        self.assertEqual(checker(None).check(recipe()).status, Status.ERROR)

    def test_should_report_rate_limit_when_quota_exhausted(self) -> None:
        routes = {"https://api.github.com/": empty_response(403, {"x-ratelimit-remaining": "0"})}
        report = PackageChecker(GithubLookup(routed_fetch(routes), None), None).check(recipe())
        self.assertEqual((report.status, report.detail), (Status.ERROR, "rate limit"))

    def test_should_include_sha1_when_hash_function_is_given(self) -> None:
        report = checker("v1.1", lambda url: "abc123").check(recipe())
        self.assertEqual(report.candidate_sha1, "abc123")

    def test_should_keep_outdated_status_when_hashing_fails(self) -> None:
        def failing(url: str) -> str:
            raise FetchError("boom")

        report = checker("v1.1", failing).check(recipe())
        self.assertEqual((report.status, report.candidate_sha1), (Status.OUTDATED, None))
        self.assertIn("sha1", report.detail)
