import tempfile
import unittest
from pathlib import Path

from pisi_bump_bot.errors import RecipesUnavailableError
from pisi_bump_bot.recipes_reader import load_recipes
from tests.support import spec_xml, write_pspec

ARCHIVE = "https://github.com/o/r/archive/v1.tar.gz"


class SpecParserTest(unittest.TestCase):
    def load(self, updates: list[tuple[int, str]]):
        with tempfile.TemporaryDirectory() as directory:
            write_pspec(Path(directory), "a/pkg/pspec.xml", spec_xml("pkg", updates, ARCHIVE))
            return load_recipes(Path(directory)).recipes[0]

    def test_should_pick_highest_release_when_it_is_not_first(self) -> None:
        recipe = self.load([(1, "1.0"), (3, "3.0"), (2, "2.0")])
        self.assertEqual((recipe.current_version, recipe.current_release), ("3.0", 3))

    def test_should_pick_first_listed_when_newest_is_first(self) -> None:
        self.assertEqual(self.load([(2, "2.0"), (1, "1.0")]).current_version, "2.0")

    def test_should_return_empty_version_when_history_is_missing(self) -> None:
        self.assertEqual(self.load([]).current_version, "")

    def test_should_read_archive_urls_from_source(self) -> None:
        self.assertEqual(self.load([(1, "1.0")]).archive_urls, (ARCHIVE,))


class LoadRecipesTest(unittest.TestCase):
    def test_should_skip_oldpackage_and_git_directories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in ("x/pspec.xml", "0oldpackage/y/pspec.xml", ".git/z/pspec.xml"):
                write_pspec(root, path, spec_xml("p", [(1, "1")], ARCHIVE))
            paths = [recipe.recipe_path for recipe in load_recipes(root).recipes]
        self.assertEqual(paths, ["x/pspec.xml"])

    def test_should_keep_duplicate_names_as_separate_recipes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_pspec(root, "util/admin/ventoy/pspec.xml", spec_xml("ventoy", [(2, "1.1")], ARCHIVE))
            write_pspec(root, "hardware/disk/ventoy/pspec.xml", spec_xml("ventoy", [(1, "1.0")], ARCHIVE))
            recipes = load_recipes(root).recipes
        self.assertEqual(sorted(r.recipe_path for r in recipes), ["hardware/disk/ventoy/pspec.xml", "util/admin/ventoy/pspec.xml"])

    def test_should_report_unreadable_path_when_xml_is_broken(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_pspec(root, "ok/pspec.xml", spec_xml("ok", [(1, "1")], ARCHIVE))
            (root / "bad").mkdir()
            (root / "bad" / "pspec.xml").write_text("<PISI", encoding="utf-8")
            load = load_recipes(root)
        self.assertEqual((len(load.recipes), load.unreadable_paths), (1, ("bad/pspec.xml",)))

    def test_should_read_recipe_when_file_starts_with_blank_line_and_bom(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "a").mkdir()
            body = '<?xml version="1.0"?><PISI>' + spec_xml("a", [(1, "1")], ARCHIVE) + "</PISI>"
            (root / "a" / "pspec.xml").write_bytes(b"\xef\xbb\xbf\n" + body.encode("utf-8"))
            load = load_recipes(root)
        self.assertEqual(len(load.recipes), 1)

    def test_should_raise_when_directory_is_missing(self) -> None:
        with self.assertRaises(RecipesUnavailableError):
            load_recipes(Path("/nonexistent/contrib"))

    def test_should_raise_when_no_pspec_exists(self) -> None:
        with tempfile.TemporaryDirectory() as directory, self.assertRaises(RecipesUnavailableError):
            load_recipes(Path(directory))
