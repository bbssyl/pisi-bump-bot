import unittest

from pisi_bump_bot.version_compare import compare_versions, extract_version_text, normalize_version


class NormalizeVersionTest(unittest.TestCase):
    def test_should_strip_leading_v_when_tag_has_v_prefix(self) -> None:
        self.assertEqual(normalize_version("v1.2.3"), (1, 2, 3))

    def test_should_strip_release_prefix_when_tag_starts_with_release(self) -> None:
        self.assertEqual(normalize_version("release-20210321"), (20210321,))

    def test_should_strip_repository_prefix_when_given(self) -> None:
        self.assertEqual(normalize_version("lzfse-1.0", ("lzfse",)), (1, 0))

    def test_should_parse_underscore_separated_tag_when_tag_uses_underscores(self) -> None:
        self.assertEqual(normalize_version("V_3_14_1"), (3, 14, 1))

    def test_should_ignore_trailing_suffix_when_tag_has_platform_suffix(self) -> None:
        self.assertEqual(normalize_version("release-2.9.12-linux4"), (2, 9, 12))

    def test_should_return_none_when_tag_has_no_digits(self) -> None:
        self.assertIsNone(normalize_version("latest"))

    def test_should_return_none_when_tag_is_prerelease(self) -> None:
        self.assertIsNone(normalize_version("1.0.0-rc1"))

    def test_should_return_none_when_text_is_empty(self) -> None:
        self.assertIsNone(normalize_version(""))


class CompareVersionsTest(unittest.TestCase):
    def test_should_treat_missing_segments_as_zero_when_lengths_differ(self) -> None:
        self.assertEqual(compare_versions((1, 0), (1, 0, 0)), 0)

    def test_should_compare_numerically_when_segments_have_different_widths(self) -> None:
        self.assertEqual(compare_versions((1, 10), (1, 9)), 1)

    def test_should_return_negative_when_left_is_older(self) -> None:
        self.assertEqual(compare_versions((0, 9, 6), (0, 9, 6, 1)), -1)


class ExtractVersionTextTest(unittest.TestCase):
    def test_should_return_matched_text_when_tag_contains_version(self) -> None:
        self.assertEqual(extract_version_text("v0.9.6.12"), "0.9.6.12")
