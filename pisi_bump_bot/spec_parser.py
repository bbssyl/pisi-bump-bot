import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass


@dataclass(frozen=True)
class PackageRecipe:
    name: str
    current_version: str
    current_release: int
    archive_urls: tuple[str, ...]
    recipe_path: str


def parse_release(value: str | None) -> int:
    return int(value) if value and value.isdigit() else 0


def release_number(update: ElementTree.Element) -> int:
    return parse_release(update.get("release"))


def newest_update(element: ElementTree.Element) -> ElementTree.Element | None:
    updates = element.findall("History/Update")
    return max(updates, key=release_number) if updates else None


def parse_spec_file(element: ElementTree.Element, recipe_path: str) -> PackageRecipe | None:
    name = (element.findtext("Source/Name") or "").strip()
    if not name:
        return None
    update = newest_update(element)
    version = (update.findtext("Version") or "").strip() if update is not None else ""
    release = release_number(update) if update is not None else 0
    urls = tuple(
        archive.text.strip() for archive in element.findall("Source/Archive") if archive.text
    )
    return PackageRecipe(name, version, release, urls, recipe_path)
