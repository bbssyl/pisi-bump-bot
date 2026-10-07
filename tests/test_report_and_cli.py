import contextlib
import io
import tempfile
import unittest
from pathlib import Path

from pisi_bump_bot.cli import Runtime, main
from pisi_bump_bot.new_updates import load_previous_outdated, render_new_updates
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.report_writer import render_json, render_markdown
from pisi_bump_bot.index_consistency import IndexConsistency
from tests.support import json_response, routed_fetch, spec_xml, write_index, write_pspec

API = "https://api.github.com/repos/"


def sample_report(latest: str = "v2") -> Report:
    packages = (
        PackageReport("beta", Status.CURRENT, "1.0", "v1.0", "o/beta", "https://r/beta"),
        PackageReport("alpha", Status.OUTDATED, "1.0", latest, "o/alpha", "https://r/alpha", "https://c/alpha"),
        PackageReport("gamma", Status.UNSUPPORTED, "1.0", detail="x|y"),
    )
    consistency = IndexConsistency(True, None, ("zzz/pspec.xml",), ())
    return Report("abc1234", tuple(sorted(packages, key=lambda p: p.name)), consistency)


class MarkdownTest(unittest.TestCase):
    def test_should_put_outdated_table_before_collapsed_sections(self) -> None:
        text = render_markdown(sample_report())
        self.assertLess(text.index("| alpha |"), text.index("<details>"))

    def test_should_list_missing_packages_when_index_is_incomplete(self) -> None:
        self.assertIn("`zzz/pspec.xml`", render_markdown(sample_report()))

    def test_should_escape_pipe_characters_in_cells(self) -> None:
        self.assertIn("x\\|y", render_markdown(sample_report()))

    def test_should_render_identical_output_when_report_is_unchanged(self) -> None:
        self.assertEqual(render_json(sample_report()), render_json(sample_report()))

    def test_should_report_index_failure_when_error_is_set(self) -> None:
        report = Report(None, (), IndexConsistency(True, "index okunamadı: x", (), ()))
        self.assertIn("index okunamadı: x", render_markdown(report))

    def test_should_show_source_commit_in_header(self) -> None:
        self.assertIn("commit `abc1234`", render_markdown(sample_report()))


class NewUpdatesTest(unittest.TestCase):
    def test_should_return_empty_text_when_nothing_changed(self) -> None:
        self.assertEqual(render_new_updates(sample_report(), {"alpha": "v2"}), "")

    def test_should_list_package_when_it_was_not_outdated_before(self) -> None:
        self.assertIn("**alpha**", render_new_updates(sample_report(), {}))

    def test_should_list_package_when_latest_version_increased(self) -> None:
        self.assertIn("**alpha**", render_new_updates(sample_report("v3"), {"alpha": "v2"}))

    def test_should_return_none_when_previous_file_is_missing(self) -> None:
        self.assertIsNone(load_previous_outdated(Path("/nonexistent/previous.json")))


class CliTest(unittest.TestCase):
    def build_root(self, directory: str) -> Path:
        root = Path(directory) / "contrib"
        archive = "https://github.com/ttcdt/mp-5.x/archive/refs/tags/5.55.tar.gz"
        write_pspec(root, "editor/mp/pspec.xml", spec_xml("mp", [(1, "5.0"), (2, "5.55")], archive))
        write_pspec(root, "editor/other/pspec.xml", spec_xml("other", [(1, "1.0")], "https://example.org/x.tar.gz"))
        write_index(root, [spec_xml("mp", [(2, "5.2")], archive, "editor/mp/pspec.xml")])
        return root

    def build_runtime(self) -> Runtime:
        routes = {f"{API}ttcdt/mp-5.x/releases/latest": json_response({"tag_name": "6.0"})}
        return Runtime(fetch=routed_fetch(routes), hash_archive=lambda url: "sha", token=None)

    def run_cli(self, directory: str, runtime: Runtime, extra: list[str] | None = None) -> int:
        base = Path(directory)
        argv = ["--recipes-dir", str(base / "contrib"), "--json", str(base / "r.json"),
                "--markdown", str(base / "r.md"), "--source-commit", "abc1234", *(extra or [])]
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            return main(argv, runtime)

    def test_should_write_reports_and_exit_zero_when_recipes_are_available(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.build_root(directory)
            code = self.run_cli(directory, self.build_runtime())
            text = (Path(directory) / "r.md").read_text(encoding="utf-8")
        self.assertEqual(code, 0)
        self.assertIn("| mp | `editor/mp/pspec.xml` | 5.55 | 6.0 |", text)
        self.assertIn("| mp | `editor/mp/pspec.xml` | 5.55 | 5.2 |", text)
        self.assertIn("commit `abc1234`", text)

    def test_should_exit_non_zero_when_recipes_dir_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            code = self.run_cli(directory, self.build_runtime())
            written = (Path(directory) / "r.json").exists()
        self.assertEqual((code, written), (1, False))

    def test_should_produce_identical_files_and_empty_updates_when_run_twice(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            self.build_root(directory)
            self.run_cli(directory, self.build_runtime())
            first = (base / "r.json").read_text(encoding="utf-8"), (base / "r.md").read_text(encoding="utf-8")
            extra = ["--previous", str(base / "r.json"), "--new-updates", str(base / "new.md")]
            self.run_cli(directory, self.build_runtime(), extra)
            second = (base / "r.json").read_text(encoding="utf-8"), (base / "r.md").read_text(encoding="utf-8")
            updates = (base / "new.md").read_text(encoding="utf-8")
        self.assertEqual((first, updates), (second, ""))

    def test_should_list_update_when_previous_report_had_older_latest(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            self.build_root(directory)
            self.run_cli(directory, self.build_runtime())
            previous = (base / "r.json").read_text(encoding="utf-8").replace('"6.0"', '"5.9"')
            (base / "prev.json").write_text(previous, encoding="utf-8")
            self.run_cli(directory, self.build_runtime(), ["--previous", str(base / "prev.json"), "--new-updates", str(base / "new.md")])
            updates = (base / "new.md").read_text(encoding="utf-8")
        self.assertIn("**mp**", updates)
