import re

VERSION_PATTERN = re.compile(r"\d+(?:[._-]\d+)*")
SEGMENT_SPLIT = re.compile(r"[._-]")
PRERELEASE_PATTERN = re.compile(r"^[-_.]?(alpha|beta|rc|pre|dev|snapshot|nightly)", re.IGNORECASE)
LEADING_WORDS = ("release-", "release_")


def strip_known_prefixes(raw: str, prefixes: tuple[str, ...]) -> str:
    text = raw.strip()
    for prefix in sorted(prefixes, key=len, reverse=True):
        for separator in ("-", "_"):
            candidate = f"{prefix.lower()}{separator}"
            if prefix and text.lower().startswith(candidate):
                text = text[len(candidate):]
    for word in LEADING_WORDS:
        if text.lower().startswith(word):
            text = text[len(word):]
    return text


def extract_version_text(raw: str, prefixes: tuple[str, ...] = ()) -> str | None:
    text = strip_known_prefixes(raw, prefixes)
    match = VERSION_PATTERN.search(text)
    if match is None:
        return None
    if PRERELEASE_PATTERN.match(text[match.end():]):
        return None
    return match.group(0)


def normalize_version(raw: str, prefixes: tuple[str, ...] = ()) -> tuple[int, ...] | None:
    version_text = extract_version_text(raw, prefixes)
    if version_text is None:
        return None
    return tuple(int(segment) for segment in SEGMENT_SPLIT.split(version_text))


def compare_versions(left: tuple[int, ...], right: tuple[int, ...]) -> int:
    length = max(len(left), len(right))
    padded_left = left + (0,) * (length - len(left))
    padded_right = right + (0,) * (length - len(right))
    return (padded_left > padded_right) - (padded_left < padded_right)
