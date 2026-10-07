import unittest

from pisi_bump_bot.github_upstream import GithubLookup
from pisi_bump_bot.package_checker import PackageChecker
from pisi_bump_bot.report_model import Status
from pisi_bump_bot.spec_parser import PackageRecipe
from tests.support import json_response, routed_fetch

OLD_URL = "https://github.com/o/r/releases/download/v1.0/r-linux-amd64-1.0.AppImage"
LATEST = "https://api.github.com/repos/o/r/releases/latest"


def check(payload: dict, url: str = OLD_URL):
    checker = PackageChecker(GithubLookup(routed_fetch({LATEST: json_response(payload)}), None), None)
    return checker.check(PackageRecipe("r", "1.0", 1, (url,), "r/pspec.xml"))


def release(*names: str) -> dict:
    return {"tag_name": "v1.1", "assets": [{"name": name} for name in names]}


class CandidateChoiceTest(unittest.TestCase):
    def test_should_use_release_asset_when_url_is_release_download(self) -> None:
        report = check(release("r-linux-amd64-1.1.AppImage", "r-linux-amd64-1.1.AppImage.zsync", "r-win64-1.1.zip"))
        self.assertEqual(report.candidate_url, "https://github.com/o/r/releases/download/v1.1/r-linux-amd64-1.1.AppImage")

    def test_should_refuse_with_reason_when_no_asset_fits(self) -> None:
        report = check(release("r-win64-1.1.zip"))
        self.assertEqual(
            (report.status, report.candidate_url, report.detail),
            (Status.OUTDATED, None, "uygun dosya bulunamadı (adaylar: r-win64-1.1.zip)"),
        )

    def test_should_substitute_when_release_payload_has_no_asset_list(self) -> None:
        report = check({"tag_name": "v1.1"})
        self.assertEqual(report.candidate_url, "https://github.com/o/r/releases/download/v1.1/r-linux-amd64-1.1.AppImage")

    def test_should_keep_substitution_when_url_is_auto_generated_archive(self) -> None:
        url = "https://github.com/o/r/archive/refs/tags/v1.0.tar.gz"
        report = check(release("r-1.1.zip"), url)
        self.assertEqual(report.candidate_url, "https://github.com/o/r/archive/refs/tags/v1.1.tar.gz")

    def test_should_quote_asset_name_when_it_has_special_characters(self) -> None:
        old = "https://github.com/o/r/releases/download/v1.0/r_1.0 x86_64.AppImage"
        report = check(release("r_1.1 x86_64.AppImage"), old)
        self.assertTrue(report.candidate_url.endswith("/r_1.1%20x86_64.AppImage"))
