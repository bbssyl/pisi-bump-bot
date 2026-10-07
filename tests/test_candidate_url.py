import unittest

from pisi_bump_bot.candidate_url import build_candidate_url


class BuildCandidateUrlTest(unittest.TestCase):
    def test_should_replace_tag_and_file_version_when_brave_style(self) -> None:
        url = "https://github.com/brave/brave-browser/releases/download/v1.93.129/brave-browser-1.93.129-linux-amd64.zip"

        result = build_candidate_url(url, "v1.93.129", "v1.94.1", "1.93.129", "1.94.1")

        self.assertEqual(
            result,
            "https://github.com/brave/brave-browser/releases/download/v1.94.1/brave-browser-1.94.1-linux-amd64.zip",
        )

    def test_should_update_dotted_filename_when_tag_uses_underscores(self) -> None:
        url = "https://github.com/atari800/atari800/releases/download/ATARI800_5_2_0/atari800-5.2.0-src.tgz"

        result = build_candidate_url(url, "ATARI800_5_2_0", "ATARI800_7_2_1", "5.2.0", "7.2.1")

        self.assertEqual(
            result,
            "https://github.com/atari800/atari800/releases/download/ATARI800_7_2_1/atari800-7.2.1-src.tgz",
        )

    def test_should_keep_file_name_when_it_has_no_version(self) -> None:
        url = "https://github.com/OpenRA/d2/releases/download/release-20250330/Dune2000-linux.zip"

        result = build_candidate_url(url, "release-20250330", "release-20250601", "20250330", "20250601")

        self.assertEqual(
            result, "https://github.com/OpenRA/d2/releases/download/release-20250601/Dune2000-linux.zip"
        )

    def test_should_replace_release_prefixed_tag_when_github_desktop_style(self) -> None:
        url = "https://github.com/shiftkey/desktop/releases/download/release-3.4.13-linux1/GitHubDesktop-linux-x86_64-3.4.13-linux1.AppImage"

        result = build_candidate_url(url, "release-3.4.13-linux1", "release-3.4.14-linux1", "3.4.13", "3.4.14")

        self.assertEqual(
            result,
            "https://github.com/shiftkey/desktop/releases/download/release-3.4.14-linux1/GitHubDesktop-linux-x86_64-3.4.14-linux1.AppImage",
        )

    def test_should_replace_tag_inside_archive_file_name_when_gnofract4d_style(self) -> None:
        url = "https://github.com/fract4d/gnofract4d/archive/v4.4.tar.gz"

        result = build_candidate_url(url, "v4.4", "v4.5", "4.4", "4.5")

        self.assertEqual(result, "https://github.com/fract4d/gnofract4d/archive/v4.5.tar.gz")

    def test_should_replace_version_in_appimage_name_when_freecad_style(self) -> None:
        url = "https://github.com/FreeCAD/FreeCAD/releases/download/1.0.1/FreeCAD_1.0.1-conda-Linux-x86_64-py311.AppImage"

        result = build_candidate_url(url, "1.0.1", "1.0.2", "1.0.1", "1.0.2")

        self.assertEqual(
            result,
            "https://github.com/FreeCAD/FreeCAD/releases/download/1.0.2/FreeCAD_1.0.2-conda-Linux-x86_64-py311.AppImage",
        )

    def test_should_not_touch_longer_version_when_old_version_is_prefix(self) -> None:
        url = "https://example.org/dl/tool-1.10.2.tar.gz"

        result = build_candidate_url(url, "v1.1", "v1.2", "1.1", "1.2")

        self.assertEqual(result, url)

    def test_should_return_url_unchanged_when_nothing_matches(self) -> None:
        url = "https://example.org/dl/latest.zip"

        result = build_candidate_url(url, "v1", "v2", "1", "2")

        self.assertEqual(result, url)


if __name__ == "__main__":
    unittest.main()
