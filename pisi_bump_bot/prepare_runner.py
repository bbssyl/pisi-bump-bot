import re
import shutil
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from pisi_bump_bot.archive_download import ArchiveDownloadError, DownloadResult
from pisi_bump_bot.build_state import BuildEntry, EntryStatus, State, package_dir
from pisi_bump_bot.candidate_url import build_candidate_url
from pisi_bump_bot.errors import BotError
from pisi_bump_bot.github_upstream import parse_github_archive
from pisi_bump_bot.pspec_updater import PreparedUpdate, PspecUpdateError, UpdateRequest, prepare_update
from pisi_bump_bot.recipes_reader import read_recipe
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.spec_parser import PackageRecipe
from pisi_bump_bot.version_compare import extract_version_text

DEFAULT_LIMIT = 10
SAFE_DIRECTORY = re.compile(r"^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$")
KEPT_STATUSES = (Status.OUTDATED, Status.ERROR)
Downloader = Callable[[str], DownloadResult]


class PrepareFailure(BotError):
    pass


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


def derive_new_version(package: PackageReport, current_version: str) -> str:
    prefixes = (package.upstream.split("/")[-1] if package.upstream else "", package.name)
    text = extract_version_text(package.latest_version or "", prefixes)
    if text is None:
        raise PrepareFailure("yeni sürüm etiketten okunamadı")
    return re.sub(r"[_-]", ".", text) if "." in current_version else text


def read_text_exact(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", newline="")
    except (OSError, UnicodeDecodeError) as error:
        raise PrepareFailure(f"{path.name} okunamadı") from error


def load_recipe(settings: PrepareSettings, package: PackageReport) -> PackageRecipe:
    recipe = read_recipe(settings.recipes_dir, package.recipe_path)
    if recipe is None or not recipe.archive_urls:
        raise PrepareFailure("pspec.xml okunamadı veya arşiv URL'si yok")
    return recipe


def resolve_candidate(package: PackageReport, recipe: PackageRecipe, new_version: str) -> str:
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
        raise PrepareFailure(str(error)) from error


def prepare_package(
    package: PackageReport, settings: PrepareSettings, download: Downloader
) -> PreparedUpdate:
    directory = package_dir(package.recipe_path)
    recipe = load_recipe(settings, package)
    new_version = derive_new_version(package, recipe.current_version)
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


def attempt(package: PackageReport, settings: PrepareSettings, download: Downloader) -> BuildEntry:
    directory = package_dir(package.recipe_path)
    version = package.latest_version or ""
    try:
        prepared = prepare_package(package, settings, download)
    except PrepareFailure as error:
        remove_prepared(settings.output_dir, directory)
        return BuildEntry(version, EntryStatus.PREPARE_FAILED, settings.run_date, reason=str(error))
    write_prepared(settings.output_dir, directory, prepared.new_pspec_text, prepared.unified_diff)
    return BuildEntry(version, EntryStatus.PREPARED, settings.run_date)


def is_pending_build(entry: BuildEntry, settings: PrepareSettings, directory: str) -> bool:
    return entry.status is EntryStatus.PREPARED and (settings.output_dir / directory / "pspec.xml").is_file()


def run_prepare(report: Report, state: State, settings: PrepareSettings, download: Downloader) -> PrepareOutcome:
    current = prune(report, state, settings.output_dir)
    build_list: list[str] = []
    attempted = 0
    for package in candidates(report):
        if attempted >= settings.limit:
            break
        directory = package_dir(package.recipe_path)
        entry = current.get(directory)
        if entry is not None and entry.version == package.latest_version:
            pending = is_pending_build(entry, settings, directory)
            attempted += pending
            build_list += [directory] if pending else []
            continue
        attempted += 1
        current[directory] = attempt(package, settings, download)
        build_list += [directory] if current[directory].status is EntryStatus.PREPARED else []
    return PrepareOutcome(current, tuple(build_list))
