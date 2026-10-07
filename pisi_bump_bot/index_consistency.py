import lzma
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from pathlib import Path

from pisi_bump_bot.spec_parser import PackageRecipe, parse_spec_file

INDEX_FILE_NAMES = ("pisi-index.xml.xz", "pisi-index.xml")


@dataclass(frozen=True)
class IndexMismatch:
    name: str
    recipe_path: str
    pspec_version: str
    index_version: str


@dataclass(frozen=True)
class IndexConsistency:
    found: bool
    error: str | None
    missing_from_index: tuple[str, ...]
    version_mismatches: tuple[IndexMismatch, ...]


def not_found() -> IndexConsistency:
    return IndexConsistency(False, None, (), ())


def failed(reason: str) -> IndexConsistency:
    return IndexConsistency(True, reason, (), ())


def locate_index(root: Path) -> Path | None:
    return next((root / name for name in INDEX_FILE_NAMES if (root / name).is_file()), None)


def read_index_versions(index_path: Path) -> dict[str, str]:
    content = index_path.read_bytes()
    if index_path.suffix == ".xz":
        content = lzma.decompress(content)
    versions: dict[str, str] = {}
    for element in ElementTree.fromstring(content).findall("SpecFile"):
        recipe_path = (element.findtext("Source/SourceURI") or "").strip()
        recipe = parse_spec_file(element, recipe_path)
        if recipe is not None and recipe_path:
            versions[recipe_path] = recipe.current_version
    return versions


def compare_with_index(recipes: tuple[PackageRecipe, ...], versions: dict[str, str]) -> IndexConsistency:
    missing = tuple(sorted(r.recipe_path for r in recipes if r.recipe_path not in versions))
    mismatches = tuple(
        IndexMismatch(r.name, r.recipe_path, r.current_version, versions[r.recipe_path])
        for r in sorted(recipes, key=lambda recipe: recipe.recipe_path)
        if r.recipe_path in versions and versions[r.recipe_path] != r.current_version
    )
    return IndexConsistency(True, None, missing, mismatches)


def check_index_consistency(root: Path, recipes: tuple[PackageRecipe, ...]) -> IndexConsistency:
    index_path = locate_index(root)
    if index_path is None:
        return not_found()
    try:
        versions = read_index_versions(index_path)
    except (lzma.LZMAError, ElementTree.ParseError, OSError) as error:
        return failed(f"index okunamadı: {error}")
    return compare_with_index(recipes, versions)
