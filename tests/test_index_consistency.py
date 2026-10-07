import tempfile
import unittest
from pathlib import Path

from pisi_bump_bot.index_consistency import check_index_consistency
from pisi_bump_bot.spec_parser import PackageRecipe
from tests.support import spec_xml, write_index

ARCHIVE = "https://github.com/o/r/archive/v1.tar.gz"


def recipe(path: str, version: str) -> PackageRecipe:
    return PackageRecipe("pkg", version, 1, (ARCHIVE,), path)


class IndexConsistencyTest(unittest.TestCase):
    def check(self, entries: list[str], recipes: tuple[PackageRecipe, ...], compressed: bool = True):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            if entries is not None:
                write_index(root, entries, compressed)
            return check_index_consistency(root, recipes)

    def test_should_list_missing_recipes_and_version_mismatches(self) -> None:
        entries = [spec_xml("a", [(1, "1.0")], ARCHIVE, "a/pspec.xml")]
        recipes = (recipe("a/pspec.xml", "2.0"), recipe("b/pspec.xml", "1.0"))
        result = self.check(entries, recipes)
        self.assertEqual(result.missing_from_index, ("b/pspec.xml",))
        self.assertEqual((result.version_mismatches[0].pspec_version, result.version_mismatches[0].index_version), ("2.0", "1.0"))

    def test_should_use_highest_release_from_index_history(self) -> None:
        entries = [spec_xml("a", [(1, "1.0"), (2, "2.0")], ARCHIVE, "a/pspec.xml")]
        result = self.check(entries, (recipe("a/pspec.xml", "2.0"),))
        self.assertEqual(result.version_mismatches, ())

    def test_should_read_plain_index_when_xz_is_absent(self) -> None:
        entries = [spec_xml("a", [(1, "1.0")], ARCHIVE, "a/pspec.xml")]
        result = self.check(entries, (recipe("a/pspec.xml", "1.0"),), compressed=False)
        self.assertEqual((result.found, result.missing_from_index), (True, ()))

    def test_should_report_not_found_when_no_index_exists(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = check_index_consistency(Path(directory), ())
        self.assertFalse(result.found)

    def test_should_report_error_when_index_is_corrupt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "pisi-index.xml.xz").write_bytes(b"junk")
            result = check_index_consistency(root, ())
        self.assertIsNotNone(result.error)
