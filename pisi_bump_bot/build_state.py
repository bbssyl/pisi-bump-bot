import json
from dataclasses import asdict, dataclass
from enum import StrEnum
from pathlib import Path

from pisi_bump_bot.errors import BotError


class EntryStatus(StrEnum):
    PREPARED = "hazir"
    PREPARE_FAILED = "hazirlanamadi"
    BUILT = "basarili"
    BUILD_FAILED = "basarisiz"


@dataclass(frozen=True)
class BuildEntry:
    version: str
    status: EntryStatus
    date: str
    reason: str | None = None
    run_id: str | None = None
    artifact_name: str | None = None


State = dict[str, BuildEntry]


def package_dir(recipe_path: str) -> str:
    return recipe_path.removesuffix("/pspec.xml")


def entry_from_document(document: dict) -> BuildEntry:
    return BuildEntry(
        version=document["version"],
        status=EntryStatus(document["status"]),
        date=document["date"],
        reason=document.get("reason"),
        run_id=document.get("run_id"),
        artifact_name=document.get("artifact_name"),
    )


def load_state(path: Path) -> State:
    if not path.is_file():
        return {}
    try:
        packages = json.loads(path.read_text(encoding="utf-8"))["packages"]
        return {key: entry_from_document(value) for key, value in packages.items()}
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise BotError(f"durum dosyası okunamadı: {error}") from error


def render_state(state: State) -> str:
    packages = {key: {**asdict(state[key]), "status": state[key].status.value} for key in sorted(state)}
    return json.dumps({"packages": packages}, ensure_ascii=False, indent=2) + "\n"


def save_state(path: Path, state: State) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(render_state(state), encoding="utf-8")
