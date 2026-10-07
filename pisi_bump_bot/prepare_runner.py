import re
import shutil
from collections.abc import Callable
from dataclasses import dataclass, replace
from pathlib import Path

from pisi_bump_bot.archive_download import ArchiveDownloadError, DownloadResult
from pisi_bump_bot.build_state import BuildEntry, EntryStatus, State, package_dir
from pisi_bump_bot.candidate_url import build_candidate_url, derive_new_version
from pisi_bump_bot.errors import BotError
from pisi_bump_bot.failure_policy import BUILD_LOST_REASON, failure_outcome, next_attempts, transient_outcome
from pisi_bump_bot.github_upstream import parse_github_archive
from pisi_bump_bot.prepare_queue import QueueItem, QueueKind, build_queue, is_exhausted_pending
from pisi_bump_bot.pspec_updater import PreparedUpdate, PspecUpdateError, UpdateRequest, prepare_update
from pisi_bump_bot.recipes_reader import read_recipe
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.spec_parser import PackageRecipe
from pisi_bump_bot.text_files import read_text_preserving_newlines

DEFAULT_LIMIT = 10
SAFE_DIRECTORY = re.compile(r"^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$")
KEPT_STATUSES = (Status.OUTDATED, Status.ERROR)
Downloader = Callable[[str], DownloadResult]


class PrepareFailure(BotError):
    def __init__(self, message: str, transient: bool = False) -> None:
        super().__init__(message)
        self.transient = transient


@dataclass(frozen=True)
class PrepareSettings:
    recipes_dir: Path
    output_dir: Path
    run_date: str
    limit: int = DEFAULT_LIMIT


@dataclass(frozen=True)
class PrepareOutcome:
    state: State
    build_list: tuple[str, ...]
    queue: tuple[QueueItem, ...] = ()


def new_version_for(package: PackageReport, current_version: str) -> str:
    prefixes = (package.upstream.split("/")[-1] if package.upstream else "", package.name)
    version = derive_new_version(package.latest_version or "", prefixes, current_version)
    if version is None:
        raise PrepareFailure("yeni sürüm etiketten okunamadı")
    return version


def read_text_exact(path: Path) -> str:
    try:
        return read_text_preserving_newlines(path)
    except (OSError, UnicodeDecodeError) as error:
        raise PrepareFailure(f"{path.name} okunamadı") from error


def load_recipe(settings: PrepareSettings, package: PackageReport) -> PackageRecipe:
    recipe = read_recipe(settings.recipes_dir, package.recipe_path)
    if recipe is None or not recipe.archive_urls:
        raise PrepareFailure("pspec.xml okunamadı veya arşiv URL'si yok")
    return recipe


def resolve_candidate(package: PackageReport, recipe: PackageRecipe, new_version: str) -> str:
    if package.candidate_url:
        return package.candidate_url
    if package.detail:
        raise PrepareFailure(package.detail)
    archive_url = recipe.archive_urls[0]
    archive = parse_github_archive(archive_url)
    if archive is None or package.latest_version is None:
        raise PrepareFailure("GitHub arşiv URL'si çözümlenemedi")
    candidate = build_candidate_url(
        archive_url, archive.tag, package.latest_version, recipe.current_version, new_version
    )
    if candidate == archive_url:
        raise PrepareFailure("aday arşiv URL'si üretilemedi")
    return candidate


def download_archive(download: Downloader, url: str) -> DownloadResult:
    try:
        return download(url)
    except ArchiveDownloadError as error:
        raise PrepareFailure(str(error), error.transient) from error


def prepare_package(
    package: PackageReport, settings: PrepareSettings, download: Downloader
) -> PreparedUpdate:
    directory = package_dir(package.recipe_path)
    recipe = load_recipe(settings, package)
    new_version = new_version_for(package, recipe.current_version)
    candidate = resolve_candidate(package, recipe, new_version)
    result = download_archive(download, candidate)
    source = settings.recipes_dir / directory
    actions = source / "actions.py"
    request = UpdateRequest(
        directory, read_text_exact(source / "pspec.xml"),
        read_text_exact(actions) if actions.is_file() else None,
        new_version, candidate, result.sha1, settings.run_date, recipe.archive_urls[0],
    )
    try:
        return prepare_update(request)
    except PspecUpdateError as error:
        raise PrepareFailure(str(error)) from error


def write_prepared(output_dir: Path, directory: str, pspec_text: str, diff_text: str) -> None:
    target = output_dir / directory
    target.mkdir(parents=True, exist_ok=True)
    (target / "pspec.xml").write_text(pspec_text, encoding="utf-8", newline="")
    (target / "pspec.diff").write_text(diff_text, encoding="utf-8", newline="")


def remove_prepared(output_dir: Path, directory: str) -> None:
    shutil.rmtree(output_dir / directory, ignore_errors=True)
    parent = (output_dir / directory).parent
    while parent != output_dir and parent.is_dir() and not any(parent.iterdir()):
        parent.rmdir()
        parent = parent.parent


def existing_directories(output_dir: Path) -> list[str]:
    return sorted(path.parent.relative_to(output_dir).as_posix() for path in output_dir.rglob("pspec.xml"))


def prune(report: Report, state: State, output_dir: Path) -> State:
    keep = {package_dir(p.recipe_path) for p in report.packages if p.status in KEPT_STATUSES}
    for directory in existing_directories(output_dir) if output_dir.is_dir() else []:
        if directory not in keep:
            remove_prepared(output_dir, directory)
    return {key: entry for key, entry in state.items() if key in keep}


def candidates(report: Report) -> list[PackageReport]:
    eligible = [
        p for p in report.with_status(Status.OUTDATED)
        if p.recipe_path and SAFE_DIRECTORY.match(package_dir(p.recipe_path)) and p.latest_version
    ]
    return sorted(eligible, key=lambda p: p.recipe_path)


def attempt(package: PackageReport, settings: PrepareSettings, download: Downloader, previous: BuildEntry | None) -> BuildEntry:
    directory = package_dir(package.recipe_path)
    version = package.latest_version or ""
    attempts = next_attempts(previous, version)
    try:
        prepared = prepare_package(package, settings, download)
    except PrepareFailure as error:
        remove_prepared(settings.output_dir, directory)
        status, reason = failure_outcome(str(error), error.transient, attempts)
        return BuildEntry(version, status, settings.run_date, reason=reason, attempts=attempts)
    write_prepared(settings.output_dir, directory, prepared.new_pspec_text, prepared.unified_diff)
    return BuildEntry(version, EntryStatus.PREPARED, settings.run_date, attempts=attempts)


def expire_lost_builds(state: State, settings: PrepareSettings) -> State:
    expired = {}
    for directory, entry in state.items():
        if is_exhausted_pending(entry, settings.output_dir, directory):
            status, reason = transient_outcome(BUILD_LOST_REASON, entry.attempts)
            entry = replace(entry, status=status, reason=reason, date=settings.run_date)
        expired[directory] = entry
    return expired


def process(item: QueueItem, current: State, settings: PrepareSettings, download: Downloader) -> BuildEntry:
    directory = package_dir(item.package.recipe_path)
    entry = current.get(directory)
    if item.kind is QueueKind.PENDING_BUILD and entry is not None:
        return replace(entry, attempts=entry.attempts + 1, date=settings.run_date)
    return attempt(item.package, settings, download, entry)


def run_prepare(report: Report, state: State, settings: PrepareSettings, download: Downloader) -> PrepareOutcome:
    current = expire_lost_builds(prune(report, state, settings.output_dir), settings)
    queue = build_queue(candidates(report), current, settings.output_dir, settings.limit)
    build_list: list[str] = []
    for item in queue:
        directory = package_dir(item.package.recipe_path)
        current[directory] = process(item, current, settings, download)
        build_list += [directory] if current[directory].status is EntryStatus.PREPARED else []
    return PrepareOutcome(current, tuple(build_list), tuple(queue))
