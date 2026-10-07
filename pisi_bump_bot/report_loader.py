import json
from pathlib import Path

from pisi_bump_bot.errors import BotError
from pisi_bump_bot.index_consistency import IndexConsistency, IndexMismatch
from pisi_bump_bot.report_model import PackageReport, Report, Status

PACKAGE_FIELDS = (
    "name", "current_version", "latest_version", "upstream", "release_url",
    "candidate_url", "candidate_sha1", "detail", "recipe_path",
)


def package_from_document(document: dict) -> PackageReport:
    values = {key: document[key] for key in PACKAGE_FIELDS if key in document}
    return PackageReport(status=Status(document["status"]), **values)


def consistency_from_document(document: dict) -> IndexConsistency:
    mismatches = tuple(IndexMismatch(**item) for item in document["version_mismatches"])
    return IndexConsistency(
        document["found"], document["error"], tuple(document["missing_from_index"]), mismatches
    )


def load_report(path: Path) -> Report:
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
        packages = tuple(package_from_document(item) for item in document["packages"])
        consistency = consistency_from_document(document["index_consistency"])
        return Report(document["source_commit"], packages, consistency)
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise BotError(f"rapor okunamadı: {error}") from error
