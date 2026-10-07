import json
import lzma
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path

from pisi_bump_bot.http_client import HttpResponse


def json_response(payload: object, status: int = 200) -> HttpResponse:
    return HttpResponse(status, {}, json.dumps(payload).encode("utf-8"))


def empty_response(status: int, headers: Mapping[str, str] | None = None) -> HttpResponse:
    return HttpResponse(status, dict(headers or {}), b"")


def routed_fetch(routes: Mapping[str, HttpResponse], calls: list[str] | None = None) -> Callable:
    def fetch(url: str, headers: Mapping[str, str]) -> HttpResponse:
        if calls is not None:
            calls.append(url)
        for prefix, response in routes.items():
            if url.startswith(prefix):
                return response
        return empty_response(404)

    return fetch


def spec_xml(name: str, updates: Sequence[tuple[int, str]], archive: str, source_uri: str | None = None) -> str:
    history = "".join(
        f'<Update release="{release}"><Date>2026-01-01</Date><Version>{version}</Version></Update>'
        for release, version in updates
    )
    uri = f"<SourceURI>{source_uri}</SourceURI>" if source_uri else ""
    return (
        f"<Source><Name>{name}</Name><Archive type=\"targz\" sha1sum=\"x\">{archive}</Archive>{uri}</Source>"
        f"<History>{history}</History>"
    )


def write_pspec(root: Path, recipe_path: str, content: str) -> None:
    target = root / recipe_path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(f"<PISI>{content}</PISI>", encoding="utf-8")


def write_index(root: Path, entries: Sequence[str], compressed: bool = True) -> None:
    body = "<PISI>" + "".join(f"<SpecFile>{entry}</SpecFile>" for entry in entries) + "</PISI>"
    if compressed:
        (root / "pisi-index.xml.xz").write_bytes(lzma.compress(body.encode("utf-8")))
    else:
        (root / "pisi-index.xml").write_text(body, encoding="utf-8")
