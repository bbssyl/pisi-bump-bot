import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from pisi_bump_bot.build_columns import BuildLinks
from pisi_bump_bot.build_results import BuildResult, load_results, merge_results, render_results_comment
from pisi_bump_bot.build_state import BuildEntry, EntryStatus, load_state, render_state, save_state
from pisi_bump_bot.cli import main
from pisi_bump_bot.index_consistency import IndexConsistency
from pisi_bump_bot.report_loader import load_report
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.report_writer import render_json, render_markdown
from tests.test_report_and_cli import sample_report

WEB = "https://github.com/o/r"


def entry(status: EntryStatus, **fields: str) -> BuildEntry:
    return BuildEntry("v2", status, "2026-10-07", **fields)


class StateTest(unittest.TestCase):
    def test_should_round_trip_state_when_saved_and_loaded(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "s" / "builds.json"
            state = {"a/b": entry(EntryStatus.PREPARE_FAILED, reason="x")}
            save_state(path, state)
            self.assertEqual(load_state(path), state)

    def test_should_return_empty_state_when_file_is_missing(self) -> None:
        self.assertEqual(load_state(Path("/nonexistent/builds.json")), {})

    def test_should_render_keys_sorted_when_state_has_many_entries(self) -> None:
        text = render_state({"b": entry(EntryStatus.PREPARED), "a": entry(EntryStatus.PREPARED)})
        self.assertLess(text.index('"a"'), text.index('"b"'))


def report_with_recipe_path() -> Report:
    package = PackageReport("alpha", Status.OUTDATED, "1.0", "v2", "o/alpha", recipe_path="alpha/pspec.xml")
    return Report("abc", (package,), IndexConsistency(True, None, (), ()))


class ColumnsTest(unittest.TestCase):
    def render(self, state: dict, **options: object) -> str:
        return render_markdown(report_with_recipe_path(), BuildLinks(state, WEB, **options))

    def test_should_show_pending_and_diff_link_when_prepared(self) -> None:
        text = self.render({"alpha": entry(EntryStatus.PREPARED)})
        self.assertIn(f"[pspec.diff]({WEB}/blob/HEAD/hazir/alpha/pspec.diff) | bekliyor |", text)

    def test_should_use_relative_diff_link_when_requested(self) -> None:
        text = self.render({"alpha": entry(EntryStatus.PREPARED)}, relative_files=True)
        self.assertIn("[pspec.diff](hazir/alpha/pspec.diff)", text)

    def test_should_show_reason_when_preparation_failed(self) -> None:
        text = self.render({"alpha": entry(EntryStatus.PREPARE_FAILED, reason="indirme başarısız")})
        self.assertIn("otomatik hazırlanamadı: indirme başarısız", text)

    def test_should_link_run_when_build_failed(self) -> None:
        text = self.render({"alpha": entry(EntryStatus.BUILD_FAILED, run_id="77")})
        self.assertIn(f"[❌]({WEB}/actions/runs/77)", text)

    def test_should_ignore_entry_when_version_differs(self) -> None:
        stale = BuildEntry("v1", EntryStatus.BUILT, "d", run_id="1")
        self.assertNotIn("✅", self.render({"alpha": stale}))

    def test_should_keep_old_columns_when_no_links_given(self) -> None:
        self.assertNotIn("Derleme", render_markdown(sample_report()))


class MergeTest(unittest.TestCase):
    def result(self, version: str = "v2") -> BuildResult:
        return BuildResult("alpha", version, EntryStatus.BUILT, "9", "derleme-alpha")

    def test_should_apply_result_when_version_matches_prepared_entry(self) -> None:
        state, applied = merge_results({"alpha": entry(EntryStatus.PREPARED)}, [self.result()], "2026-10-08")
        self.assertEqual((state["alpha"].status, state["alpha"].run_id, len(applied)), (EntryStatus.BUILT, "9", 1))

    def test_should_ignore_result_when_version_is_stale(self) -> None:
        state, applied = merge_results({"alpha": entry(EntryStatus.PREPARED)}, [self.result("v1")], "d")
        self.assertEqual((state["alpha"].status, applied), (EntryStatus.PREPARED, []))

    def test_should_load_results_and_skip_broken_files_when_directory_has_junk(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "result-a.json").write_text(json.dumps(
                {"recipe_path": "alpha", "version": "v2", "status": "basarili", "run_id": 5, "artifact_name": "x"}))
            (base / "result-b.json").write_text("{bozuk")
            (base / "result-c.json").write_text(json.dumps({"recipe_path": "c", "version": "v", "status": "?"}))
            results = load_results(base)
        self.assertEqual([r.recipe_path for r in results], ["alpha"])

    def test_should_render_comment_with_run_link_when_results_applied(self) -> None:
        self.assertIn(f"({WEB}/actions/runs/9)", render_results_comment([self.result()], WEB))

    def test_should_render_empty_comment_when_nothing_applied(self) -> None:
        self.assertEqual(render_results_comment([], WEB), "")


class SubcommandTest(unittest.TestCase):
    def run_main(self, argv: list[str]) -> int:
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return main(argv)

    def test_should_load_report_identically_when_rendered_from_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "r.json"
            path.write_text(render_json(sample_report()), encoding="utf-8")
            self.assertEqual(render_json(load_report(path)), render_json(sample_report()))

    def test_should_render_markdown_when_render_command_runs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            (base / "r.json").write_text(render_json(sample_report()), encoding="utf-8")
            code = self.run_main(["render", "--report", str(base / "r.json"), "--state", str(base / "s.json"),
                                  "--markdown", str(base / "o.md"), "--relative-files"])
            text = (base / "o.md").read_text(encoding="utf-8")
        self.assertEqual(code, 0)
        self.assertIn("Hazır pspec", text)

    def test_should_fail_when_report_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            code = self.run_main(["render", "--report", "/nonexistent.json", "--state", "/nonexistent.json",
                                  "--markdown", str(Path(directory) / "o.md")])
        self.assertEqual(code, 1)

    def test_should_write_state_and_comment_when_merge_command_runs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            save_state(base / "s.json", {"alpha": entry(EntryStatus.PREPARED)})
            (base / "res").mkdir()
            (base / "res" / "result-alpha.json").write_text(json.dumps(
                {"recipe_path": "alpha", "version": "v2", "status": "basarisiz", "run_id": "3"}))
            code = self.run_main(["merge-results", "--state", str(base / "s.json"), "--results-dir", str(base / "res"),
                                  "--comment", str(base / "c.md"), "--repo-web-url", WEB])
            status = load_state(base / "s.json")["alpha"].status
            comment = (base / "c.md").read_text(encoding="utf-8")
        self.assertEqual((code, status), (0, EntryStatus.BUILD_FAILED))
        self.assertIn("❌", comment)
