import argparse
import datetime
import json
import sys
from collections.abc import Sequence
from pathlib import Path

from pisi_bump_bot.build_columns import BuildLinks
from pisi_bump_bot.build_results import load_results, merge_results, render_results_comment
from pisi_bump_bot.build_state import load_state, save_state
from pisi_bump_bot.errors import BotError
from pisi_bump_bot.prepare_runner import DEFAULT_LIMIT, Downloader, PrepareSettings, run_prepare
from pisi_bump_bot.report_loader import load_report
from pisi_bump_bot.report_writer import render_markdown

SUBCOMMANDS = ("prepare", "merge-results", "render")


def today() -> str:
    return datetime.datetime.now(datetime.timezone.utc).date().isoformat()


def build_subcommand_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="pisi_bump_bot")
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--report", type=Path, required=True)
    prepare.add_argument("--recipes-dir", dest="recipes_dir", type=Path, required=True)
    prepare.add_argument("--state", type=Path, required=True)
    prepare.add_argument("--output-dir", dest="output_dir", type=Path, required=True)
    prepare.add_argument("--build-list", dest="build_list", type=Path, required=True)
    prepare.add_argument("--run-date", dest="run_date", default=None)
    prepare.add_argument("--limit", type=int, default=DEFAULT_LIMIT)
    merge = commands.add_parser("merge-results")
    merge.add_argument("--state", type=Path, required=True)
    merge.add_argument("--results-dir", dest="results_dir", type=Path, required=True)
    merge.add_argument("--comment", type=Path, required=True)
    merge.add_argument("--run-date", dest="run_date", default=None)
    merge.add_argument("--repo-web-url", dest="repo_web_url", default=None)
    render = commands.add_parser("render")
    render.add_argument("--report", type=Path, required=True)
    render.add_argument("--state", type=Path, required=True)
    render.add_argument("--markdown", type=Path, required=True)
    render.add_argument("--repo-web-url", dest="repo_web_url", default=None)
    render.add_argument("--relative-files", dest="relative_files", action="store_true")
    return parser


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def run_prepare_command(arguments: argparse.Namespace, download: Downloader) -> None:
    report = load_report(arguments.report)
    settings = PrepareSettings(
        arguments.recipes_dir, arguments.output_dir, arguments.run_date or today(), arguments.limit
    )
    outcome = run_prepare(report, load_state(arguments.state), settings, download)
    save_state(arguments.state, outcome.state)
    write_text(arguments.build_list, json.dumps(list(outcome.build_list)))
    print(f"derlenecek paket sayısı: {len(outcome.build_list)}", file=sys.stderr)


def run_merge_command(arguments: argparse.Namespace) -> None:
    state, applied = merge_results(
        load_state(arguments.state), load_results(arguments.results_dir), arguments.run_date or today()
    )
    save_state(arguments.state, state)
    write_text(arguments.comment, render_results_comment(applied, arguments.repo_web_url))


def run_render_command(arguments: argparse.Namespace) -> None:
    links = BuildLinks(load_state(arguments.state), arguments.repo_web_url, arguments.relative_files)
    write_text(arguments.markdown, render_markdown(load_report(arguments.report), links))


def run_subcommand(argv: Sequence[str], download: Downloader) -> int:
    arguments = build_subcommand_parser().parse_args(argv)
    try:
        if arguments.command == "prepare":
            run_prepare_command(arguments, download)
        elif arguments.command == "merge-results":
            run_merge_command(arguments)
        else:
            run_render_command(arguments)
    except BotError as error:
        print(f"{arguments.command} başarısız: {error}", file=sys.stderr)
        return 1
    return 0
