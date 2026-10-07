import json
from pathlib import Path

from pisi_bump_bot.report_model import PackageReport, Report, Status
from pisi_bump_bot.version_compare import compare_versions, normalize_version


def package_key(name: str, recipe_path: str | None) -> str:
    return recipe_path or name


def load_previous_outdated(path: Path) -> dict[str, str | None] | None:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
        packages = document["packages"]
        return {
            package_key(p["name"], p.get("recipe_path")): p.get("latest_version")
            for p in packages
            if p.get("status") == Status.OUTDATED.value
        }
    except (OSError, ValueError, KeyError, TypeError):
        return None


def is_newer(latest: str | None, previous: str | None) -> bool:
    if latest == previous:
        return False
    parsed_latest = normalize_version(latest or "")
    parsed_previous = normalize_version(previous or "")
    if parsed_latest is None or parsed_previous is None:
        return True
    return compare_versions(parsed_latest, parsed_previous) > 0


def is_new_update(package: PackageReport, previous: dict[str, str | None]) -> bool:
    key = package_key(package.name, package.recipe_path)
    if key not in previous:
        return True
    return is_newer(package.latest_version, previous[key])


def render_new_updates(report: Report, previous: dict[str, str | None]) -> str:
    fresh = [p for p in report.with_status(Status.OUTDATED) if is_new_update(p, previous)]
    if not fresh:
        return ""
    lines = ["## Yeni güncellemeler", ""]
    for package in fresh:
        reference = f"[{package.latest_version}]({package.release_url})" if package.release_url else package.latest_version
        lines.append(f"- **{package.name}** (`{package.recipe_path}`): {package.current_version} -> {reference}")
    return "\n".join(lines) + "\n"
