import tempfile
import unittest
from pathlib import Path

from pisi_bump_bot.archive_download import ArchiveDownloadError, DownloadResult
from pisi_bump_bot.build_state import BuildEntry, EntryStatus
from pisi_bump_bot.index_consistency import IndexConsistency
from pisi_bump_bot.prepare_runner import PrepareSettings, run_prepare
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.text_files import read_text_preserving_newlines

FIXTURES = Path(__file__).parent / "fixtures" / "pspec"
FAKE_SHA1 = "0123456789abcdef0123456789abcdef01234567"
ATARI = "game/emulator/atari800"
BRAVE = "network/browser/brave"


def outdated(name: str, directory: str, latest: str, upstream: str) -> PackageReport:
    return PackageReport(
        name, Status.OUTDATED, "x", latest, upstream, recipe_path=f"{directory}/pspec.xml"
    )


def make_report(*packages: PackageReport) -> Report:
    return Report("abc", tuple(packages), IndexConsistency(True, None, (), ()))


def fake_download(url: str) -> DownloadResult:
    return DownloadResult(url, FAKE_SHA1, 10)


def failing_download(url: str) -> DownloadResult:
    raise ArchiveDownloadError("indirme başarısız (HTTP 404)")


class PrepareRunnerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        base = Path(self.temp.name)
        for directory, fixture in ((ATARI, "atari800.xml"), (BRAVE, "brave.xml")):
            target = base / "contrib" / directory
            target.mkdir(parents=True)
            (target / "pspec.xml").write_text(read_text_preserving_newlines(FIXTURES / fixture), encoding="utf-8", newline="")
        self.settings = PrepareSettings(base / "contrib", base / "hazir", "2026-10-07")
        self.report = make_report(
            outdated("brave-browser", BRAVE, "v1.94.1", "brave/brave-browser"),
            outdated("atari800", ATARI, "ATARI800_7_2_1", "atari800/atari800"),
        )

    def test_should_prepare_files_in_path_order_when_downloads_succeed(self) -> None:
        outcome = run_prepare(self.report, {}, self.settings, fake_download)
        self.assertEqual(outcome.build_list, (ATARI, BRAVE))
        self.assertTrue((self.settings.output_dir / ATARI / "pspec.diff").is_file())

    def test_should_use_converted_version_and_new_url_when_tag_has_underscores(self) -> None:
        run_prepare(self.report, {}, self.settings, fake_download)
        text = (self.settings.output_dir / ATARI / "pspec.xml").read_text(encoding="utf-8")
        self.assertIn("ATARI800_7_2_1/atari800-7.2.1-src.tgz", text)
        self.assertIn("<Version>7.2.1</Version>", text)

    def test_should_record_failure_reason_when_download_fails(self) -> None:
        outcome = run_prepare(self.report, {}, self.settings, failing_download)
        entry = outcome.state[ATARI]
        self.assertEqual((entry.status, entry.reason, outcome.build_list), (
            EntryStatus.PREPARE_FAILED, "indirme başarısız (HTTP 404)", ()))
        self.assertFalse((self.settings.output_dir / ATARI).exists())

    def test_should_not_retry_when_same_version_already_failed(self) -> None:
        state = {ATARI: BuildEntry("ATARI800_7_2_1", EntryStatus.PREPARE_FAILED, "d", reason="x")}
        calls: list[str] = []
        run_prepare(make_report(self.report.packages[1]), state, self.settings, lambda u: calls.append(u) or fake_download(u))
        self.assertEqual(calls, [])

    def test_should_retry_when_upstream_version_changed(self) -> None:
        state = {ATARI: BuildEntry("ATARI800_7_2_0", EntryStatus.PREPARE_FAILED, "d", reason="x")}
        outcome = run_prepare(make_report(self.report.packages[1]), state, self.settings, fake_download)
        self.assertEqual(outcome.state[ATARI].status, EntryStatus.PREPARED)

    def test_should_not_rebuild_when_build_already_finished(self) -> None:
        state = {BRAVE: BuildEntry("v1.94.1", EntryStatus.BUILT, "d", run_id="1")}
        outcome = run_prepare(make_report(self.report.packages[0]), state, self.settings, fake_download)
        self.assertEqual(outcome.build_list, ())

    def test_should_respect_limit_when_many_candidates(self) -> None:
        limited = PrepareSettings(self.settings.recipes_dir, self.settings.output_dir, "d", limit=1)
        outcome = run_prepare(self.report, {}, limited, fake_download)
        self.assertEqual(outcome.build_list, (ATARI,))

    def test_should_prune_files_and_state_when_package_is_no_longer_outdated(self) -> None:
        run_prepare(self.report, {}, self.settings, fake_download)
        state = {ATARI: BuildEntry("ATARI800_7_2_1", EntryStatus.PREPARED, "d"), BRAVE: BuildEntry("v1.94.1", EntryStatus.BUILT, "d")}
        current = PackageReport("brave-browser", Status.CURRENT, "1", recipe_path=f"{BRAVE}/pspec.xml")
        outcome = run_prepare(make_report(self.report.packages[1], current), state, self.settings, fake_download)
        self.assertEqual(set(outcome.state), {ATARI})
        self.assertFalse((self.settings.output_dir / "network").exists())

    def test_should_requeue_prepared_package_when_build_never_reported(self) -> None:
        run_prepare(self.report, {}, self.settings, fake_download)
        state = {BRAVE: BuildEntry("v1.94.1", EntryStatus.PREPARED, "d")}
        outcome = run_prepare(make_report(self.report.packages[0]), state, self.settings, failing_download)
        self.assertEqual(outcome.build_list, (BRAVE,))
