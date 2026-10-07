import os
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from pathlib import Path

from pisi_bump_bot.errors import RecipesUnavailableError
from pisi_bump_bot.spec_parser import PackageRecipe, parse_spec_file

PSPEC_FILE_NAME = "pspec.xml"
LEADING_NOISE = b"\xef\xbb\xbf \t\r\n"
EXCLUDED_DIRECTORIES = frozenset({"0oldpackage", ".git"})


@dataclass(frozen=True)
class RecipeLoad:
    recipes: tuple[PackageRecipe, ...]
    unreadable_paths: tuple[str, ...]


def find_pspec_paths(root: Path) -> list[str]:
    found: list[str] = []
    for directory, subdirectories, files in os.walk(root):
        subdirectories[:] = sorted(name for name in subdirectories if name not in EXCLUDED_DIRECTORIES)
        if PSPEC_FILE_NAME in files:
            found.append(Path(directory, PSPEC_FILE_NAME).relative_to(root).as_posix())
    return sorted(found)


def read_recipe(root: Path, recipe_path: str) -> PackageRecipe | None:
    try:
        element = ElementTree.fromstring((root / recipe_path).read_bytes().lstrip(LEADING_NOISE))
    except (ElementTree.ParseError, OSError):
        return None
    return parse_spec_file(element, recipe_path)


def load_recipes(root: Path) -> RecipeLoad:
    if not root.is_dir():
        raise RecipesUnavailableError(f"{root}: dizin bulunamadı")
    paths = find_pspec_paths(root)
    if not paths:
        raise RecipesUnavailableError(f"{root}: pspec.xml bulunamadı")
    parsed = [(path, read_recipe(root, path)) for path in paths]
    recipes = tuple(recipe for _, recipe in parsed if recipe is not None)
    unreadable = tuple(path for path, recipe in parsed if recipe is None)
    return RecipeLoad(recipes, unreadable)
