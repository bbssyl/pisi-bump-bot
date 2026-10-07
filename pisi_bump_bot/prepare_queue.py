from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path

from pisi_bump_bot.build_state import MAX_ATTEMPTS, PREPARE_LOGIC_VERSION, BuildEntry, EntryStatus, State, package_dir
from pisi_bump_bot.report_model import PackageReport


class QueueKind(StrEnum):
    NEW = "yeni"
    PENDING_BUILD = "derleme bekliyor"
    TRANSIENT_RETRY = "geçici hata tekrarı"
    LOGIC_RETRY = "mantık değişikliği tekrarı"


QUEUE_ORDER = (QueueKind.NEW, QueueKind.PENDING_BUILD, QueueKind.TRANSIENT_RETRY, QueueKind.LOGIC_RETRY)


@dataclass(frozen=True)
class QueueItem:
    package: PackageReport
    kind: QueueKind


def has_prepared_file(output_dir: Path, directory: str) -> bool:
    return (output_dir / directory / "pspec.xml").is_file()


def is_pending_build(entry: BuildEntry, output_dir: Path, directory: str) -> bool:
    return entry.status is EntryStatus.PREPARED and has_prepared_file(output_dir, directory)


def is_exhausted_pending(entry: BuildEntry, output_dir: Path, directory: str) -> bool:
    return is_pending_build(entry, output_dir, directory) and entry.attempts >= MAX_ATTEMPTS


def classify(package: PackageReport, state: State, output_dir: Path) -> QueueKind | None:
    directory = package_dir(package.recipe_path)
    entry = state.get(directory)
    if entry is None or entry.version != package.latest_version:
        return QueueKind.NEW
    if is_pending_build(entry, output_dir, directory):
        return QueueKind.PENDING_BUILD
    if entry.status is EntryStatus.PREPARED or entry.status is EntryStatus.TRANSIENT_FAILED:
        return QueueKind.TRANSIENT_RETRY
    if entry.status is EntryStatus.PREPARE_FAILED and entry.logic_version < PREPARE_LOGIC_VERSION:
        return QueueKind.LOGIC_RETRY
    return None


def build_queue(packages: list[PackageReport], state: State, output_dir: Path, limit: int) -> list[QueueItem]:
    items = [QueueItem(package, kind) for package in packages if (kind := classify(package, state, output_dir))]
    ordered = [item for kind in QUEUE_ORDER for item in items if item.kind is kind]
    return ordered[:limit]
