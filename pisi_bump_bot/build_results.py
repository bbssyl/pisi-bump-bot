import json
from dataclasses import dataclass, replace
from pathlib import Path

from pisi_bump_bot.build_columns import MARKS
from pisi_bump_bot.build_state import BuildEntry, EntryStatus, State

RESULT_PATTERN = "result-*.json"
RESULT_STATUSES = {"basarili": EntryStatus.BUILT, "basarisiz": EntryStatus.BUILD_FAILED}


@dataclass(frozen=True)
class BuildResult:
    recipe_path: str
    version: str
    status: EntryStatus
    run_id: str | None
    artifact_name: str | None


def result_from_document(document: dict) -> BuildResult | None:
    status = RESULT_STATUSES.get(document.get("status"))
    if status is None or not document.get("recipe_path") or not document.get("version"):
        return None
    return BuildResult(
        document["recipe_path"], document["version"], status,
        str(document["run_id"]) if document.get("run_id") else None, document.get("artifact_name"),
    )


def read_result(path: Path) -> BuildResult | None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
        return result_from_document(document) if isinstance(document, dict) else None
    except (OSError, ValueError):
        return None


def load_results(directory: Path) -> list[BuildResult]:
    found = (read_result(path) for path in sorted(directory.rglob(RESULT_PATTERN)))
    return sorted((result for result in found if result is not None), key=lambda r: r.recipe_path)


def apply_result(entry: BuildEntry, result: BuildResult, run_date: str) -> BuildEntry:
    return replace(
        entry, status=result.status, date=run_date, reason=None,
        run_id=result.run_id, artifact_name=result.artifact_name,
    )


def merge_results(state: State, results: list[BuildResult], run_date: str) -> tuple[State, list[BuildResult]]:
    merged = dict(state)
    applied: list[BuildResult] = []
    for result in results:
        entry = merged.get(result.recipe_path)
        if entry is not None and entry.version == result.version and entry.status is EntryStatus.PREPARED:
            merged[result.recipe_path] = apply_result(entry, result, run_date)
            applied.append(result)
    return merged, applied


def render_results_comment(applied: list[BuildResult], web_url: str | None) -> str:
    if not applied:
        return ""
    lines = ["## Derleme sonuçları (deneme derlemesi)", ""]
    for result in applied:
        run = f"{web_url}/actions/runs/{result.run_id}" if web_url and result.run_id else None
        suffix = f" ([çalışma]({run}))" if run else ""
        lines.append(f"- {MARKS[result.status]} `{result.recipe_path}` {result.version}{suffix}")
    return "\n".join(lines) + "\n"
