import json
import tempfile
import unittest
import urllib.error
from dataclasses import replace
from pathlib import Path

from pisi_bump_bot.archive_download import ArchiveDownloadError, DownloadResult, download_sha1
from pisi_bump_bot.build_columns import BuildLinks
from pisi_bump_bot.build_results import BuildResult, merge_results
from pisi_bump_bot.build_state import BuildEntry, EntryStatus, load_state, render_state
from pisi_bump_bot.index_consistency import IndexConsistency
from pisi_bump_bot.prepare_queue import QueueKind
from pisi_bump_bot.prepare_runner import PrepareSettings, run_prepare
from pisi_bump_bot.report_model import PackageReport, Report, Status
from tests.test_prepare_runner import ATARI, BRAVE, FAKE_SHA1, outdated

LIVE_STATE = Path(__file__).parent / "fixtures" / "state" / "live-builds.json"
ATARI_TAG = "ATARI800_7_2_1"


def http_failure(code: int, headers: dict[str, str] | None = None):
    def opener(request, data=None, *, timeout):
        raise urllib.error.HTTPError(request.full_url, code, "x", headers or {}, None)

    return opener


def network_failure(error: BaseException):
    def opener(request, data=None, *, timeout):
        raise error

    return opener


def make_report(*packages: PackageReport) -> Report:
    return Report("abc", tuple(packages), IndexConsistency(True, None, (), ()))


def fail_with(reason: str, transient: bool):
    def download(url: str) -> DownloadResult:
        raise ArchiveDownloadError(reason, transient)

    return download


def succeed(url: str) -> DownloadResult:
    return DownloadResult(url, FAKE_SHA1, 10)


class DownloadClassificationTest(unittest.TestCase):
    def classify(self, opener) -> bool:
        with self.assertRaises(ArchiveDownloadError) as context:
            download_sha1("https://x/y", opener)
        return context.exception.transient

    def test_should_be_permanent_when_http_404_or_410(self) -> None:
        self.assertEqual([self.classify(http_failure(404)), self.classify(http_failure(410))], [False, False])

    def test_should_be_transient_when_http_5xx_or_429(self) -> None:
        self.assertEqual([self.classify(http_failure(503)), self.classify(http_failure(429))], [True, True])

    def test_should_be_transient_when_forbidden_by_rate_limit(self) -> None:
        self.assertTrue(self.classify(http_failure(403, {"X-RateLimit-Remaining": "0"})))

    def test_should_be_permanent_when_forbidden_without_rate_limit(self) -> None:
        self.assertFalse(self.classify(http_failure(403)))

    def test_should_be_transient_when_connection_or_timeout_fails(self) -> None:
        errors = (urllib.error.URLError("dns"), TimeoutError(), ConnectionResetError())
        self.assertTrue(all(self.classify(network_failure(error)) for error in errors))


class RetryPolicyTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        base = Path(self.temp.name)
        fixtures = Path(__file__).parent / "fixtures" / "pspec"
        for directory, fixture in ((ATARI, "atari800.xml"), (BRAVE, "brave.xml")):
            target = base / "contrib" / directory
            target.mkdir(parents=True)
            (target / "pspec.xml").write_bytes((fixtures / fixture).read_bytes())
        self.settings = PrepareSettings(base / "contrib", base / "hazir", "2026-10-07")
        self.atari = outdated("atari800", ATARI, ATARI_TAG, "atari800/atari800")
        self.brave = outdated("brave-browser", BRAVE, "v1.94.1", "brave/brave-browser")

    def prepare(self, state: dict, download, *packages: PackageReport, settings: PrepareSettings | None = None):
        return run_prepare(make_report(*packages), state, settings or self.settings, download)

    def test_should_store_transient_failure_with_attempt_count(self) -> None:
        outcome = self.prepare({}, fail_with("zaman aşımı", True), self.atari)
        entry = outcome.state[ATARI]
        self.assertEqual((entry.status, entry.attempts, entry.reason), (EntryStatus.TRANSIENT_FAILED, 1, "zaman aşımı"))

    def test_should_retry_transient_failure_on_next_run(self) -> None:
        state = {ATARI: BuildEntry(ATARI_TAG, EntryStatus.TRANSIENT_FAILED, "d", reason="x", attempts=1)}
        outcome = self.prepare(state, succeed, self.atari)
        self.assertEqual((outcome.state[ATARI].status, outcome.state[ATARI].attempts), (EntryStatus.PREPARED, 2))

    def test_should_become_permanent_when_third_attempt_fails_transiently(self) -> None:
        state = {ATARI: BuildEntry(ATARI_TAG, EntryStatus.TRANSIENT_FAILED, "d", reason="x", attempts=2)}
        entry = self.prepare(state, fail_with("zaman aşımı", True), self.atari).state[ATARI]
        self.assertEqual((entry.status, entry.reason), (EntryStatus.PREPARE_FAILED, "3 denemede de başarısız: zaman aşımı"))

    def test_should_not_retry_permanent_failure_recorded_under_current_logic(self) -> None:
        state = {ATARI: BuildEntry(ATARI_TAG, EntryStatus.PREPARE_FAILED, "d", reason="x", attempts=1)}
        self.assertEqual(self.prepare(state, succeed, self.atari).queue, ())

    def test_should_retry_old_logic_failure_once_then_keep_permanent(self) -> None:
        legacy = BuildEntry(ATARI_TAG, EntryStatus.PREPARE_FAILED, "d", reason="HTTP 404", logic_version=1)
        first = self.prepare({ATARI: legacy}, fail_with("HTTP 404", False), self.atari)
        second = self.prepare(first.state, succeed, self.atari)
        self.assertEqual((first.queue[0].kind, first.state[ATARI].logic_version, second.queue), (QueueKind.LOGIC_RETRY, 2, ()))

    def test_should_fail_permanently_when_report_has_asset_refusal(self) -> None:
        refused = replace(self.atari, detail="uygun dosya bulunamadı (adaylar: a)")
        entry = self.prepare({}, succeed, refused).state[ATARI]
        self.assertEqual((entry.status, entry.reason), (EntryStatus.PREPARE_FAILED, "uygun dosya bulunamadı (adaylar: a)"))

    def test_should_download_report_candidate_url_when_present(self) -> None:
        urls: list[str] = []
        chosen = replace(self.atari, candidate_url="https://github.com/atari800/atari800/releases/download/ATARI800_7_2_1/atari800-7.2.1-src.tgz")
        self.prepare({}, lambda url: urls.append(url) or succeed(url), chosen)
        self.assertEqual(urls, [chosen.candidate_url])

    def test_should_fill_leftover_slots_by_priority_new_then_pending_then_retry(self) -> None:
        wide = PrepareSettings(self.settings.recipes_dir, self.settings.output_dir, "d", limit=2)
        self.prepare({}, succeed, self.brave)
        state = {
            BRAVE: BuildEntry("v1.94.1", EntryStatus.PREPARED, "d", attempts=1),
            ATARI: BuildEntry(ATARI_TAG, EntryStatus.TRANSIENT_FAILED, "d", reason="x", attempts=1),
        }
        new = outdated("zeta", "zz/zeta", "v1", "o/zeta")
        outcome = self.prepare(state, succeed, self.atari, self.brave, new, settings=wide)
        self.assertEqual([item.kind for item in outcome.queue], [QueueKind.NEW, QueueKind.PENDING_BUILD])

    def test_should_expire_pending_build_when_no_result_after_three_attempts(self) -> None:
        self.prepare({}, succeed, self.brave)
        state = {BRAVE: BuildEntry("v1.94.1", EntryStatus.PREPARED, "d", attempts=3)}
        entry = self.prepare(state, succeed, self.brave).state[BRAVE]
        self.assertEqual((entry.status, entry.reason), (EntryStatus.PREPARE_FAILED, "3 denemede de başarısız: derleme sonucu alınamadı"))


class LiveStateMigrationTest(unittest.TestCase):
    def test_should_load_live_state_with_legacy_logic_version_and_zero_attempts(self) -> None:
        state = load_state(LIVE_STATE)
        entry = state["multimedia/graphics/freecad"]
        self.assertEqual((len(state), entry.status, entry.logic_version, entry.attempts), (10, EntryStatus.PREPARE_FAILED, 1, 0))

    def test_should_round_trip_live_state_with_new_fields(self) -> None:
        state = load_state(LIVE_STATE)
        document = json.loads(render_state(state))["packages"]["multimedia/graphics/freecad"]
        self.assertEqual((document["logic_version"], document["attempts"]), (1, 0))

    def test_should_queue_only_old_404_entries_when_live_state_is_replayed(self) -> None:
        state = load_state(LIVE_STATE)
        packages = [
            outdated(directory.rsplit("/", 1)[-1], directory, entry.version, "o/r")
            for directory, entry in sorted(state.items())
        ]
        with tempfile.TemporaryDirectory() as directory:
            settings = PrepareSettings(Path(directory) / "c", Path(directory) / "h", "d")
            outcome = run_prepare(make_report(*packages), state, settings, succeed)
        retried = sorted(item.package.recipe_path.removesuffix("/pspec.xml") for item in outcome.queue)
        self.assertEqual(retried, [key for key, e in state.items() if e.status is EntryStatus.PREPARE_FAILED])
        self.assertTrue(all(item.kind is QueueKind.LOGIC_RETRY for item in outcome.queue))


class BuildResultPolicyTest(unittest.TestCase):
    def merge(self, attempts: int):
        state = {"alpha": BuildEntry("v2", EntryStatus.PREPARED, "d", attempts=attempts)}
        result = BuildResult("alpha", "v2", EntryStatus.TRANSIENT_FAILED, "9", None)
        return merge_results(state, [result], "d")[0]["alpha"]

    def test_should_mark_transient_when_build_produced_no_status(self) -> None:
        self.assertEqual(self.merge(1).status, EntryStatus.TRANSIENT_FAILED)

    def test_should_become_permanent_when_build_has_no_status_on_third_attempt(self) -> None:
        entry = self.merge(3)
        self.assertEqual(entry.status, EntryStatus.PREPARE_FAILED)
        self.assertTrue(entry.reason.startswith("3 denemede de başarısız: "))


class BuildCellTest(unittest.TestCase):
    def cell(self, entry: BuildEntry) -> str | None:
        return BuildLinks({"alpha": entry}).build_cell("alpha/pspec.xml", "v2")

    def test_should_show_transient_message_with_attempt_counter(self) -> None:
        entry = BuildEntry("v2", EntryStatus.TRANSIENT_FAILED, "d", reason="zaman aşımı", attempts=2)
        self.assertEqual(self.cell(entry), "geçici hata, tekrar denenecek (2/3): zaman aşımı")

    def test_should_show_not_prepared_message_when_permanent(self) -> None:
        entry = BuildEntry("v2", EntryStatus.PREPARE_FAILED, "d", reason="HTTP 404")
        self.assertEqual(self.cell(entry), "otomatik hazırlanamadı: HTTP 404")
