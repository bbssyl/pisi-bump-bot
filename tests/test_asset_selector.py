import json
import unittest
from pathlib import Path

from pisi_bump_bot.asset_filters import architecture_family, file_type, is_helper_file, system_family
from pisi_bump_bot.asset_selector import VersionPair, select_asset

FIXTURES = Path(__file__).parent / "fixtures" / "assets"


def load_assets(name: str) -> list[str]:
    return json.loads((FIXTURES / f"{name}.json").read_text(encoding="utf-8"))


class RealReleaseSelectionTest(unittest.TestCase):
    def pick(self, fixture: str, old_name: str, old: str, new: str) -> str | None:
        return select_asset(old_name, VersionPair(old, new), load_assets(fixture)).name

    def test_should_pick_freecad_appimage_when_conda_marker_was_dropped(self) -> None:
        picked = self.pick("freecad", "FreeCAD_1.0.1-conda-Linux-x86_64-py311.AppImage", "1.0.1", "1.1.4")
        self.assertEqual(picked, "FreeCAD_1.1.4-Linux-x86_64-py311.AppImage")

    def test_should_pick_pencil2d_amd64_when_old_name_has_no_v_prefix(self) -> None:
        picked = self.pick("pencil2d", "pencil2d-linux-amd64-0.6.6.AppImage", "0.6.6", "0.7.2")
        self.assertEqual(picked, "pencil2d-linux-amd64-v0.7.2.AppImage")

    def test_should_pick_synfig_linux64_when_date_and_hash_changed(self) -> None:
        old_name = "SynfigStudio-1.5.1-2021.10.21-linux64-2cb6c.appimage"
        picked = self.pick("synfig", old_name, "1.5.1", "1.5.5")
        self.assertEqual(picked, "SynfigStudio-1.5.5-2026.03.15-linux64-79bf7.appimage")

    def test_should_pick_iptvnator_deb_when_naming_scheme_changed(self) -> None:
        picked = self.pick("iptvnator", "iptvnator_0.15.0_amd64.deb", "0.15.0", "0.24.0")
        self.assertEqual(picked, "iptvnator-0.24.0-linux-amd64.deb")


class RefusalTest(unittest.TestCase):
    def test_should_refuse_with_candidate_list_when_only_foreign_platforms_exist(self) -> None:
        assets = ["app-1.1-win64.zip", "app-1.1-mac.dmg", "app-1.1-linux-arm64.zip", "a.zsync"]

        selection = select_asset("app-1.0-linux-x86_64.zip", VersionPair("1.0", "1.1"), assets)

        self.assertEqual(selection.name, None)
        self.assertEqual(selection.reason, "uygun dosya bulunamadı (adaylar: app-1.1-win64.zip, app-1.1-mac.dmg, app-1.1-linux-arm64.zip)")

    def test_should_refuse_when_top_two_candidates_are_within_margin(self) -> None:
        assets = ["tool-1.1-linux-x86_64-gtk.tar.gz", "tool-1.1-linux-x86_64-qt5.tar.gz"]

        selection = select_asset("tool-1.0-linux-x86_64.tar.gz", VersionPair("1.0", "1.1"), assets)

        self.assertIsNone(selection.name)
        self.assertIn("gtk", selection.reason)

    def test_should_refuse_when_best_similarity_is_below_threshold(self) -> None:
        selection = select_asset("alpha-1.0.zip", VersionPair("1.0", "2.0"), ["completely-different-name.zip"])
        self.assertIsNone(selection.name)

    def test_should_limit_listed_names_to_five(self) -> None:
        assets = [f"x-{index}-win.zip" for index in range(8)] + ["x-linux.tar.gz"]
        selection = select_asset("x-1.0-linux.zip", VersionPair("1.0", "2.0"), assets)
        self.assertEqual(selection.reason.count(","), 4)


class FilterTest(unittest.TestCase):
    def test_should_flag_helper_files_when_extension_is_checksum_or_metadata(self) -> None:
        for name in ("a.AppImage.zsync", "SHA256SUMS.txt", "a.deb.sha256", "latest-linux.yml", "a.sbom.json", "x.intoto.jsonl"):
            self.assertTrue(is_helper_file(name), name)
        self.assertFalse(is_helper_file("a.AppImage"))

    def test_should_normalize_file_types_when_compound_or_aliased(self) -> None:
        self.assertEqual(
            (file_type("A.TAR.GZ"), file_type("a.tgz"), file_type("x.AppImage"), file_type("x.tar.xz")),
            ("tar.gz", "tar.gz", "appimage", "tar.xz"),
        )

    def test_should_detect_architecture_families(self) -> None:
        names = ("a-x86_64.zip", "a-amd64.deb", "a-aarch64.zip", "a-armv7l.deb", "a-i686.zip", "a-linux32.zip", "a.zip")
        self.assertEqual(
            [architecture_family(name) for name in names], ["x64", "x64", "arm64", "arm", "x86", "x86", None]
        )

    def test_should_detect_foreign_systems_when_name_has_windows_or_mac_tokens(self) -> None:
        self.assertEqual(
            [system_family(n) for n in ("a-win64.zip", "a-macOS.dmg", "setup.exe", "a-linux64.tar.gz", "a.zip")],
            ["windows", "mac", "windows", "linux", None],
        )


class EmptyReleaseTest(unittest.TestCase):
    def test_should_say_no_candidates_when_release_has_no_assets(self) -> None:
        selection = select_asset("a-1.0.zip", VersionPair("1.0", "2.0"), [])
        self.assertEqual(selection.reason, "uygun dosya bulunamadı (adaylar: yok)")


class ProductGuardTest(unittest.TestCase):
    def select(self, old_name: str, old: str):
        return select_asset(old_name, VersionPair(old, "1.96.61"), load_assets("brave"))

    def test_should_pick_stable_zip_when_old_file_is_stable_brave(self) -> None:
        selection = self.select("brave-browser-1.93.129-linux-amd64.zip", "1.93.129")
        self.assertEqual(selection.name, "brave-browser-1.96.61-linux-amd64.zip")

    def test_should_refuse_when_old_file_is_nightly_and_release_has_other_products(self) -> None:
        selection = self.select("brave-browser-nightly-1.95.29-linux-amd64.zip", "1.95.29")
        self.assertIsNone(selection.name)
        self.assertTrue(selection.reason.startswith("uygun dosya bulunamadı (adaylar: "))

    def test_should_compare_whole_name_when_old_file_has_no_version(self) -> None:
        assets = ["zen-x86_64.AppImage", "zen-aarch64.AppImage", "other-x86_64.AppImage"]
        self.assertEqual(select_asset("zen-x86_64.AppImage", VersionPair("1.0", "2.0"), assets).name, "zen-x86_64.AppImage")
