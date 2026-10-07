import json
from dataclasses import asdict

from pisi_bump_bot.report_model import PackageReport, Report, Status

EMPTY_CELL = "-"
SOURCE_URL = "https://git.pisilinux.org/Pisilinux/contrib"
COLLAPSED_SECTIONS = (
    (Status.CURRENT, "Güncel paketler"),
    (Status.UNCOMPARABLE, "Karşılaştırılamayan paketler"),
    (Status.UNSUPPORTED, "Desteklenmeyen paketler"),
    (Status.ERROR, "Hata veren paketler"),
)


def sort_packages(packages: tuple[PackageReport, ...]) -> tuple[PackageReport, ...]:
    return tuple(sorted(packages, key=lambda p: (p.name.casefold(), p.name, p.recipe_path)))


def summary_counts(report: Report) -> dict[str, int]:
    counts = {"toplam": len(report.packages)}
    counts.update({status.value: report.count(status) for status in Status})
    counts["index_te_olmayan"] = len(report.index_consistency.missing_from_index)
    counts["index_surum_farki"] = len(report.index_consistency.version_mismatches)
    return counts


def render_json(report: Report) -> str:
    consistency = report.index_consistency
    document = {
        "source_url": SOURCE_URL,
        "source_commit": report.source_commit,
        "summary": summary_counts(report),
        "index_consistency": {
            "found": consistency.found,
            "error": consistency.error,
            "missing_from_index": list(consistency.missing_from_index),
            "version_mismatches": [asdict(mismatch) for mismatch in consistency.version_mismatches],
        },
        "packages": [{**asdict(package), "status": package.status.value} for package in report.packages],
    }
    return json.dumps(document, ensure_ascii=False, indent=2) + "\n"


def escape_cell(value: str | None) -> str:
    if not value:
        return EMPTY_CELL
    return value.replace("|", "\\|").replace("\n", " ")


def table(headers: tuple[str, ...], rows: list[tuple[str | None, ...]]) -> list[str]:
    lines = ["| " + " | ".join(headers) + " |", "|" + "|".join(" --- " for _ in headers) + "|"]
    lines.extend("| " + " | ".join(escape_cell(cell) for cell in row) + " |" for row in rows)
    return lines


def link(label: str | None, url: str | None) -> str | None:
    if not label or not url:
        return label
    return f"[{label}]({url})"


def code(value: str | None) -> str | None:
    return f"`{value}`" if value else None


def outdated_table(packages: tuple[PackageReport, ...]) -> list[str]:
    if not packages:
        return ["Eski paket yok."]
    rows = [
        (p.name, code(p.recipe_path), p.current_version, p.latest_version,
         link(p.upstream, p.release_url), code(p.candidate_url))
        for p in packages
    ]
    return table(("Paket", "Pspec", "Mevcut", "Yeni", "Kaynak", "Aday arşiv URL"), rows)


def generic_table(packages: tuple[PackageReport, ...]) -> list[str]:
    rows = [
        (p.name, code(p.recipe_path), p.current_version, p.latest_version, p.upstream, p.detail)
        for p in packages
    ]
    return table(("Paket", "Pspec", "Mevcut", "Yeni", "Kaynak", "Ayrıntı"), rows)


def details_block(title: str, count: int, body: list[str]) -> list[str]:
    return ["", "<details>", f"<summary>{title} ({count})</summary>", "", *body, "", "</details>"]


def summary_lines(report: Report) -> list[str]:
    consistency = report.index_consistency
    labels = (
        ("Toplam paket", len(report.packages)),
        ("Eski", report.count(Status.OUTDATED)),
        ("Güncel", report.count(Status.CURRENT)),
        ("Desteklenmiyor", report.count(Status.UNSUPPORTED)),
        ("Karşılaştırılamadı", report.count(Status.UNCOMPARABLE)),
        ("Hata", report.count(Status.ERROR)),
        ("Index'te olmayan", len(consistency.missing_from_index)),
        ("Index sürüm farkı", len(consistency.version_mismatches)),
    )
    return [f"- {label}: {value}" for label, value in labels]


def consistency_body(report: Report) -> list[str]:
    consistency = report.index_consistency
    if not consistency.found:
        return ["Depoda pisi-index.xml.xz veya pisi-index.xml bulunamadı."]
    if consistency.error:
        return [consistency.error]
    body = [f"Index'te olmayan paketler ({len(consistency.missing_from_index)}):", ""]
    body += [f"- `{path}`" for path in consistency.missing_from_index]
    body += ["", f"Pspec sürümü index sürümünden farklı olanlar ({len(consistency.version_mismatches)}):", ""]
    rows = [(m.name, code(m.recipe_path), m.pspec_version, m.index_version) for m in consistency.version_mismatches]
    return body + table(("Paket", "Pspec", "Pspec sürümü", "Index sürümü"), rows)


def consistency_block(report: Report) -> list[str]:
    consistency = report.index_consistency
    count = len(consistency.missing_from_index) + len(consistency.version_mismatches)
    return details_block("Index tutarsızlıkları", count, consistency_body(report))


def source_line(report: Report) -> str:
    commit = f", commit `{report.source_commit}`" if report.source_commit else ""
    return f"Kaynak: {SOURCE_URL} (pspec.xml dosyaları{commit})"


def render_markdown(report: Report) -> str:
    lines = ["# Pisi Linux contrib güncellik raporu", "", source_line(report), ""]
    lines += ["## Özet", "", *summary_lines(report), "", "## Eski paketler", ""]
    lines += outdated_table(report.with_status(Status.OUTDATED))
    for status, title in COLLAPSED_SECTIONS:
        packages = report.with_status(status)
        if packages:
            lines += details_block(title, len(packages), generic_table(packages))
    lines += consistency_block(report)
    return "\n".join(lines) + "\n"


def render_console(report: Report) -> str:
    consistency = report.index_consistency
    lines = [f"Toplam: {len(report.packages)}"]
    lines += [f"{status.value}: {report.count(status)}" for status in Status]
    lines.append(f"index'te olmayan: {len(consistency.missing_from_index)}")
    lines.append(f"index sürüm farkı: {len(consistency.version_mismatches)}")
    outdated = report.with_status(Status.OUTDATED)
    lines += [""] + [f"{p.name} [{p.recipe_path}]: {p.current_version} -> {p.latest_version}" for p in outdated]
    return "\n".join(lines)
