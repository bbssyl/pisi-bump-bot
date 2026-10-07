import re
from collections.abc import Iterable
from dataclasses import dataclass
from difflib import SequenceMatcher

from pisi_bump_bot.asset_filters import (
    is_helper_file,
    matches_architecture,
    matches_file_type,
    matches_system,
)
from pisi_bump_bot.candidate_url import version_variants

MIN_SIMILARITY = 0.6
MIN_MARGIN = 0.05
MAX_LISTED_NAMES = 5
VERSION_PLACEHOLDER = "@version@"
ARCHITECTURE_SYNONYMS = re.compile(r"(?<![a-z0-9])(?:x86[_-]64|amd64|x64|linux64|64-?bit)(?![a-z0-9])")
ARCHITECTURE_PLACEHOLDER = "@arch@"


@dataclass(frozen=True)
class VersionPair:
    old: str
    new: str


@dataclass(frozen=True)
class AssetSelection:
    name: str | None
    similarity: float
    reason: str | None = None


def mask_version(name: str, version: str) -> str:
    for variant in version_variants(version):
        pattern = rf"(?<![A-Za-z0-9])v?{re.escape(variant)}(?!\d|\.\d)"
        name = re.sub(pattern, VERSION_PLACEHOLDER, name, flags=re.IGNORECASE)
    return name


def comparable(name: str, version: str) -> str:
    masked = mask_version(name, version).lower()
    return ARCHITECTURE_SYNONYMS.sub(ARCHITECTURE_PLACEHOLDER, masked)


def product_stem(name: str, version: str) -> str:
    text = comparable(name, version)
    stem = text.partition(VERSION_PLACEHOLDER)[0]
    return re.sub(r"[-_.\s]+", "-", stem).strip("-")


def has_same_product(old_name: str, candidate: str, versions: VersionPair) -> bool:
    return product_stem(old_name, versions.old) == product_stem(candidate, versions.new)


def similarity(old_name: str, candidate: str, versions: VersionPair) -> float:
    left = comparable(old_name, versions.old)
    right = comparable(candidate, versions.new)
    return SequenceMatcher(None, left, right).ratio()


def eligible_assets(old_name: str, assets: Iterable[str], versions: VersionPair) -> list[str]:
    usable = [name for name in assets if not is_helper_file(name)]
    return [
        name for name in usable
        if matches_file_type(old_name, name)
        and matches_architecture(old_name, name)
        and matches_system(old_name, name)
        and has_same_product(old_name, name, versions)
    ]


def refusal_reason(names: Iterable[str]) -> str:
    listed = ", ".join(list(names)[:MAX_LISTED_NAMES]) or "yok"
    return f"uygun dosya bulunamadı (adaylar: {listed})"


def is_ambiguous(ranked: list[tuple[float, str]]) -> bool:
    return len(ranked) > 1 and ranked[0][0] - ranked[1][0] < MIN_MARGIN


def select_asset(old_name: str, versions: VersionPair, assets: Iterable[str]) -> AssetSelection:
    names = list(assets)
    eligible = eligible_assets(old_name, names, versions)
    ranked = sorted(((similarity(old_name, name, versions), name) for name in eligible), key=lambda r: (-r[0], r[1]))
    shown = [name for _score, name in ranked] or [name for name in names if not is_helper_file(name)]
    if not ranked or ranked[0][0] < MIN_SIMILARITY or is_ambiguous(ranked):
        best = ranked[0][0] if ranked else 0.0
        return AssetSelection(None, best, refusal_reason(shown))
    return AssetSelection(ranked[0][1], ranked[0][0])
