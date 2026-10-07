import argparse
import os
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from pisi_bump_bot.errors import BotError
from pisi_bump_bot.github_upstream import GithubLookup
from pisi_bump_bot.http_client import Fetch, HashArchive, stream_sha1, urllib_fetch
from pisi_bump_bot.index_consistency import check_index_consistency
from pisi_bump_bot.new_updates import load_previous_outdated, package_key, render_new_updates
from pisi_bump_bot.package_checker import PackageChecker
from pisi_bump_bot.recipes_reader import RecipeLoad, load_recipes
from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.report_writer import render_console, render_json, render_markdown, sort_packages

TOKEN_VARIABLE = "GITHUB_TOKEN"
UNREADABLE_DETAIL = "pspec.xml okunamadı"


@dataclass(frozen=True)
class Runtime:
    fetch: Fetch = urllib_fetch
    hash_archive: HashArchive = stream_sha1
    token: str | None = None


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="pisi_bump_bot")
    parser.add_argument("--recipes-dir", dest="recipes_dir", type=Path, required=True)
    parser.add_argument("--json", dest="json_path", type=Path, required=True)
    parser.add_argument("--markdown", dest="markdown_path", type=Path, required=True)
    parser.add_argument("--previous", dest="previous_path", type=Path)
    parser.add_argument("--new-updates", dest="new_updates_path", type=Path)
    parser.add_argument("--source-commit")
    parser.add_argument("--with-hash", action="store_true")
    return parser


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def unreadable_report(recipe_path: str) -> PackageReport:
    name = Path(recipe_path).parent.name or recipe_path
    return PackageReport(name, Status.ERROR, "", detail=UNREADABLE_DETAIL, recipe_path=recipe_path)


def check_packages(load: RecipeLoad, runtime: Runtime, with_hash: bool) -> tuple[PackageReport, ...]:
    lookup = GithubLookup(runtime.fetch, runtime.token)
    checker = PackageChecker(lookup, runtime.hash_archive if with_hash else None)
    reports = [checker.check(recipe) for recipe in load.recipes]
    reports += [unreadable_report(path) for path in load.unreadable_paths]
    return sort_packages(tuple(reports))


def build_report(arguments: argparse.Namespace, runtime: Runtime) -> Report:
    load = load_recipes(arguments.recipes_dir)
    packages = check_packages(load, runtime, arguments.with_hash)
    consistency = check_index_consistency(arguments.recipes_dir, load.recipes)
    return Report(arguments.source_commit, packages, consistency)


def write_new_updates(arguments: argparse.Namespace, report: Report) -> None:
    if arguments.new_updates_path is None:
        return
    previous = load_previous_outdated(arguments.previous_path) if arguments.previous_path else None
    if previous is None:
        print("önceki rapor yok, yeni güncelleme listesi boş", file=sys.stderr)
        previous = {package_key(p.name, p.recipe_path): p.latest_version for p in report.packages}
    write_text(arguments.new_updates_path, render_new_updates(report, previous))


def run(arguments: argparse.Namespace, runtime: Runtime) -> int:
    try:
        report = build_report(arguments, runtime)
    except BotError as error:
        print(f"tarifler okunamadı: {error}", file=sys.stderr)
        return 1
    write_text(arguments.json_path, render_json(report))
    write_text(arguments.markdown_path, render_markdown(report))
    write_new_updates(arguments, report)
    print(render_console(report))
    return 0


def main(argv: Sequence[str] | None = None, runtime: Runtime | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    active = runtime or Runtime(token=os.environ.get(TOKEN_VARIABLE))
    return run(arguments, active)
