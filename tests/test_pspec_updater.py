import unittest
import xml.etree.ElementTree as ElementTree
from pathlib import Path

from pisi_bump_bot.pspec_updater import PspecUpdateError, UpdateRequest, prepare_update
from pisi_bump_bot.text_files import read_text_preserving_newlines

FIXTURES = Path(__file__).parent / "fixtures" / "pspec"
FAKE_SHA1 = "0123456789abcdef0123456789abcdef01234567"
RUN_DATE = "2026-10-07"


def read_fixture(name: str) -> str:
    return read_text_preserving_newlines(FIXTURES / name)


def build_request(pspec_text: str, **overrides: object) -> UpdateRequest:
    fields: dict[str, object] = {
        "recipe_path": "network/browser/brave",
        "pspec_text": pspec_text,
        "actions_text": None,
        "new_version": "1.94.1",
        "new_archive_url": "https://example.org/new-1.94.1.zip",
        "new_sha1": FAKE_SHA1,
        "run_date": RUN_DATE,
    }
    fields.update(overrides)
    return UpdateRequest(**fields)


def changed_lines(diff: str, marker: str) -> list[str]:
    return [line[1:] for line in diff.splitlines() if line.startswith(marker) and line[1:2] != marker]


class BraveFixtureTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = read_fixture("brave.xml")
        self.result = prepare_update(build_request(self.text))

    def test_should_touch_only_archive_and_new_history_block_when_brave(self) -> None:
        removed = changed_lines(self.result.unified_diff, "-")
        added = changed_lines(self.result.unified_diff, "+")

        self.assertEqual(len(removed), 2)
        self.assertEqual(len(added), 9)
        self.assertIn("https://example.org/new-1.94.1.zip", added[1])

    def test_should_preserve_newline_wrapped_url_layout_when_brave(self) -> None:
        self.assertIn(
            f'<Archive sha1sum="{FAKE_SHA1}" type="binary">\n           https://example.org/new-1.94.1.zip\n        </Archive>',
            self.result.new_pspec_text,
        )

    def test_should_insert_release_four_with_author_when_brave(self) -> None:
        root = ElementTree.fromstring(self.result.new_pspec_text)
        first = root.find("History/Update")

        self.assertEqual(first.get("release"), "4")
        self.assertEqual(first.findtext("Email"), "pisi-bump-bot@users.noreply.github.com")
        self.assertEqual(first.findtext("Comment"), "Version bump to 1.94.1")
        self.assertEqual(first.findtext("Date"), RUN_DATE)

    def test_should_use_expected_diff_labels_when_brave(self) -> None:
        lines = self.result.unified_diff.splitlines()

        self.assertEqual(lines[0], "--- a/network/browser/brave/pspec.xml")
        self.assertEqual(lines[1], "+++ b/network/browser/brave/pspec.xml")

    def test_should_keep_rest_of_file_byte_identical_when_brave(self) -> None:
        old_lines = self.text.splitlines(keepends=True)
        new_lines = self.result.new_pspec_text.splitlines(keepends=True)

        self.assertEqual(old_lines[-5:], new_lines[-5:])
        self.assertEqual(len(new_lines), len(old_lines) + 7)


class MultiArchiveTest(unittest.TestCase):
    def test_should_update_only_matching_archive_when_old_url_given(self) -> None:
        text = read_fixture("multi-archive.xml")
        old_url = "http://download.brother.com/welcome/dlf101548/hl1210wcupswrapper-3.0.1-1.i386.rpm"

        result = prepare_update(build_request(text, old_archive_url=old_url))

        root = ElementTree.fromstring(result.new_pspec_text.lstrip())
        archives = root.findall("Source/Archive")
        self.assertIn("hl1210wlpr-3.0.1-1.i386.rpm", archives[0].text)
        self.assertEqual(archives[1].get("sha1sum"), FAKE_SHA1)
        self.assertIn("birden fazla Archive var, sadece eşleşen güncellendi", result.warnings)

    def test_should_raise_when_old_url_matches_nothing(self) -> None:
        text = read_fixture("multi-archive.xml")

        with self.assertRaisesRegex(PspecUpdateError, "Archive"):
            prepare_update(build_request(text, old_archive_url="https://nowhere.example/x.zip"))

    def test_should_update_first_archive_when_old_url_not_given(self) -> None:
        text = read_fixture("multi-archive.xml")
        self.assertTrue(text.startswith("\n"))

        result = prepare_update(build_request(text))

        root = ElementTree.fromstring(result.new_pspec_text.lstrip())
        self.assertEqual(root.find("Source/Archive").get("sha1sum"), FAKE_SHA1)


class OddIndentationTest(unittest.TestCase):
    def test_should_reproduce_existing_indentation_when_update_is_misindented(self) -> None:
        text = read_fixture("odd-indent.xml")

        result = prepare_update(build_request(text, new_version="5.6.0"))

        added = changed_lines(result.unified_diff, "+")
        self.assertIn('            <Update release="3">', added)
        self.assertIn("            <Date>2026-10-07</Date>", added)
        self.assertIn("        </Update>", added)

    def test_should_not_alter_other_lines_when_misindented(self) -> None:
        text = read_fixture("odd-indent.xml")

        result = prepare_update(build_request(text))

        self.assertEqual(len(changed_lines(result.unified_diff, "-")), 1)


class ActionsAndTabsTest(unittest.TestCase):
    def test_should_warn_when_actions_contains_old_version(self) -> None:
        text = read_fixture("atari800.xml")
        actions = read_fixture("atari800-actions.py.txt")
        old_version = ElementTree.fromstring(text).find("History/Update/Version").text
        self.assertIn(old_version, actions + "\n" + old_version)

        result = prepare_update(build_request(text, actions_text=f'WorkDir = "atari800-{old_version}"'))

        self.assertIn("actions.py içinde sürüm elle yazılmış, elle kontrol edilmeli", result.warnings)

    def test_should_warn_when_actions_contains_underscore_version(self) -> None:
        text = read_fixture("atari800.xml")
        old_version = ElementTree.fromstring(text).find("History/Update/Version").text
        underscored = old_version.replace(".", "_")

        result = prepare_update(build_request(text, actions_text=f"tag = '{underscored}'"))

        self.assertEqual(len(result.warnings), 1)

    def test_should_not_warn_when_actions_has_no_version(self) -> None:
        text = read_fixture("atari800.xml")

        result = prepare_update(build_request(text, actions_text=read_fixture("atari800-actions.py.txt")))

        self.assertEqual(result.warnings, ())

    def test_should_warn_and_keep_tabs_when_file_has_tabs(self) -> None:
        text = read_fixture("tabs.xml")

        result = prepare_update(build_request(text))

        self.assertIn("dosyada sekme karakteri var (Pisi kuralı: boşluk kullanılmalı)", result.warnings)
        self.assertEqual(result.new_pspec_text.count("\t"), text.count("\t"))


class FormattingQuirksTest(unittest.TestCase):
    def test_should_preserve_crlf_line_endings_when_file_uses_crlf(self) -> None:
        text = read_fixture("brave.xml").replace("\n", "\r\n")

        result = prepare_update(build_request(text))

        self.assertNotIn("\n", result.new_pspec_text.replace("\r\n", ""))

    def test_should_preserve_bom_and_missing_trailing_newline(self) -> None:
        text = "﻿" + read_fixture("brave.xml").rstrip("\n")

        result = prepare_update(build_request(text))

        self.assertTrue(result.new_pspec_text.startswith("﻿"))
        self.assertFalse(result.new_pspec_text.endswith("\n"))

    def test_should_escape_ampersand_in_new_url(self) -> None:
        text = read_fixture("brave.xml")

        result = prepare_update(build_request(text, new_archive_url="https://example.org/a?x=1&y=2"))

        self.assertIn("a?x=1&amp;y=2", result.new_pspec_text)

    def test_should_not_touch_dependencies_when_updating(self) -> None:
        text = read_fixture("brave.xml")

        result = prepare_update(build_request(text))

        self.assertFalse(any("Dependency" in line for line in changed_lines(result.unified_diff, "+")))


class ErrorTest(unittest.TestCase):
    def test_should_raise_when_pspec_is_not_well_formed(self) -> None:
        with self.assertRaisesRegex(PspecUpdateError, "okunamadı"):
            prepare_update(build_request("<PISI><Source>"))

    def test_should_raise_when_history_has_no_update(self) -> None:
        text = "<PISI><Source><Archive sha1sum=\"a\">u</Archive></Source><History></History></PISI>"

        with self.assertRaisesRegex(PspecUpdateError, "Update"):
            prepare_update(build_request(text))

    def test_should_raise_when_archive_has_no_sha1sum(self) -> None:
        text = read_fixture("brave.xml").replace(' sha1sum="87f7f1897ed922ec6898b4de3d1297ea2fd1c7ae"', "")

        with self.assertRaisesRegex(PspecUpdateError, "sha1sum"):
            prepare_update(build_request(text))


if __name__ == "__main__":
    unittest.main()
