from pathlib import Path


def read_text_preserving_newlines(path: Path) -> str:
    with open(path, encoding="utf-8", newline="") as handle:
        return handle.read()
