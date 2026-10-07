from dataclasses import dataclass
from enum import StrEnum

from pisi_bump_bot.index_consistency import IndexConsistency


class Status(StrEnum):
    OUTDATED = "eski"
    CURRENT = "guncel"
    UNSUPPORTED = "desteklenmiyor"
    UNCOMPARABLE = "karsilastirilamadi"
    ERROR = "hata"


@dataclass(frozen=True)
class PackageReport:
    name: str
    status: Status
    current_version: str
    latest_version: str | None = None
    upstream: str | None = None
    release_url: str | None = None
    candidate_url: str | None = None
    candidate_sha1: str | None = None
    detail: str | None = None
    recipe_path: str = ""


@dataclass(frozen=True)
class Report:
    source_commit: str | None
    packages: tuple[PackageReport, ...]
    index_consistency: IndexConsistency

    def count(self, status: Status) -> int:
        return sum(1 for package in self.packages if package.status is status)

    def with_status(self, status: Status) -> tuple[PackageReport, ...]:
        return tuple(package for package in self.packages if package.status is status)
