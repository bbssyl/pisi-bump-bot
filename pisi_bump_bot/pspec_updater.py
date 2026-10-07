import difflib
import html
import io
import re
import xml.etree.ElementTree as ElementTree
from dataclasses import dataclass
from xml.sax.saxutils import escape

from pisi_bump_bot.errors import BotError
from pisi_bump_bot.spec_parser import newest_update, release_number

AUTHOR_NAME = "pisi-bump-bot"
AUTHOR_EMAIL = "pisi-bump-bot@users.noreply.github.com"
ARCHIVE_PATTERN = re.compile(r"(<Archive\b[^>]*>)(\s*)([^<]*?)(\s*)(</Archive>)")
SHA1_ATTRIBUTE = re.compile(r"""(sha1sum\s*=\s*)(["'])[^"']*\2""")
HISTORY_UPDATE = re.compile(r"<History\b[^>]*>.*?(<Update\b[^>]*>)", re.DOTALL)
CHILD_TAGS = ("Date", "Version", "Comment", "Name", "Email")
DEFAULT_CHILD_INDENT = "    "


class PspecUpdateError(BotError):
    pass


@dataclass(frozen=True)
class UpdateRequest:
    recipe_path: str
    pspec_text: str
    actions_text: str | None
    new_version: str
    new_archive_url: str
    new_sha1: str
    run_date: str
    old_archive_url: str | None = None


@dataclass(frozen=True)
class PreparedUpdate:
    recipe_path: str
    new_pspec_text: str
    unified_diff: str
    warnings: tuple[str, ...]


def parse_document(text: str) -> ElementTree.Element:
    try:
        return ElementTree.fromstring(text.lstrip("﻿ \t\r\n"))
    except ElementTree.ParseError as error:
        raise PspecUpdateError(f"pspec.xml okunamadı: {error}") from error


def line_indent(text: str, position: int) -> str:
    line_start = text.rfind("\n", 0, position) + 1
    prefix = text[line_start:position]
    if prefix.strip():
        raise PspecUpdateError("History/Update satırı beklenen biçimde değil")
    return prefix


def with_new_sha1(opening: str, sha1: str) -> str:
    if SHA1_ATTRIBUTE.search(opening) is None:
        raise PspecUpdateError("Archive öğesinde sha1sum özniteliği yok")
    return SHA1_ATTRIBUTE.sub(lambda found: f"{found.group(1)}{found.group(2)}{sha1}{found.group(2)}", opening, count=1)


def matches_old_url(match: re.Match[str], request: UpdateRequest) -> bool:
    return request.old_archive_url is None or html.unescape(match.group(3)) == request.old_archive_url


def replace_archive(text: str, request: UpdateRequest) -> str:
    for match in ARCHIVE_PATTERN.finditer(text):
        if not matches_old_url(match, request):
            continue
        opening = with_new_sha1(match.group(1), request.new_sha1)
        url = escape(request.new_archive_url)
        replacement = f"{opening}{match.group(2)}{url}{match.group(4)}{match.group(5)}"
        return text[: match.start()] + replacement + text[match.end() :]
    raise PspecUpdateError("eşleşen Archive öğesi bulunamadı")


def child_indent(block: str, tag: str, fallback: str) -> str:
    match = re.search(rf"^([ \t]*)<{tag}>", block, re.MULTILINE)
    return match.group(1) if match else fallback


def build_update_block(existing_block: str, opening_indent: str, release: int, request: UpdateRequest) -> str:
    newline = "\r\n" if "\r\n" in existing_block else "\n"
    values = {
        "Date": request.run_date,
        "Version": request.new_version,
        "Comment": f"Version bump to {request.new_version}",
        "Name": AUTHOR_NAME,
        "Email": AUTHOR_EMAIL,
    }
    fallback = opening_indent + DEFAULT_CHILD_INDENT
    lines = [f'{opening_indent}<Update release="{release}">']
    for tag in CHILD_TAGS:
        indent = child_indent(existing_block, tag, fallback)
        lines.append(f"{indent}<{tag}>{escape(values[tag])}</{tag}>")
    closing = re.search(r"^([ \t]*)</Update>", existing_block, re.MULTILINE)
    lines.append(f"{closing.group(1) if closing else opening_indent}</Update>")
    return newline.join(lines) + newline


def insert_history_entry(text: str, release: int, request: UpdateRequest) -> str:
    match = HISTORY_UPDATE.search(text)
    if match is None:
        raise PspecUpdateError("History içinde Update bulunamadı")
    start = match.start(1)
    end = text.find("</Update>", start)
    if end < 0:
        raise PspecUpdateError("History/Update kapanmıyor")
    line_start = text.rfind("\n", 0, start) + 1
    block = build_update_block(text[line_start : end + len("</Update>")], line_indent(text, start), release, request)
    return text[:line_start] + block + text[line_start:]


def validate_result(new_text: str, release: int, request: UpdateRequest) -> None:
    root = parse_document(new_text)
    latest = max(root.findall("History/Update"), key=release_number)
    if release_number(latest) != release or (latest.findtext("Version") or "").strip() != request.new_version:
        raise PspecUpdateError("yeni History kaydı doğrulanamadı")
    archives = [
        (archive.get("sha1sum"), (archive.text or "").strip()) for archive in root.findall("Source/Archive")
    ]
    if (request.new_sha1, request.new_archive_url) not in archives:
        raise PspecUpdateError("Archive güncellemesi doğrulanamadı")


def version_present(text: str, version: str) -> bool:
    variants = {version, version.replace(".", "_")}
    return any(re.search(rf"(?<![\w.]){re.escape(variant)}(?![\w]|\.\d)", text) for variant in variants)


def collect_warnings(root: ElementTree.Element, request: UpdateRequest) -> tuple[str, ...]:
    warnings: list[str] = []
    latest = newest_update(root)
    old_version = (latest.findtext("Version") or "").strip() if latest is not None else ""
    if request.actions_text and old_version and version_present(request.actions_text, old_version):
        warnings.append("actions.py içinde sürüm elle yazılmış, elle kontrol edilmeli")
    if len(root.findall("Source/Archive")) > 1:
        warnings.append("birden fazla Archive var, sadece eşleşen güncellendi")
    if "\t" in request.pspec_text:
        warnings.append("dosyada sekme karakteri var (Pisi kuralı: boşluk kullanılmalı)")
    return tuple(warnings)


def make_diff(recipe_path: str, old_text: str, new_text: str) -> str:
    label = f"{recipe_path}/pspec.xml"
    old_lines = io.StringIO(old_text, newline="\n").readlines()
    new_lines = io.StringIO(new_text, newline="\n").readlines()
    diff = difflib.unified_diff(old_lines, new_lines, f"a/{label}", f"b/{label}")
    return "".join(line if line.endswith("\n") else f"{line}\n" for line in diff)


def prepare_update(request: UpdateRequest) -> PreparedUpdate:
    root = parse_document(request.pspec_text)
    releases = [release_number(update) for update in root.findall("History/Update")]
    release = max(releases, default=0) + 1
    with_archive = replace_archive(request.pspec_text, request)
    new_text = insert_history_entry(with_archive, release, request)
    validate_result(new_text, release, request)
    diff = make_diff(request.recipe_path, request.pspec_text, new_text)
    return PreparedUpdate(request.recipe_path, new_text, diff, collect_warnings(root, request))
