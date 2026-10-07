import re

HELPER_SUFFIXES = (
    ".zsync", ".sha256", ".sha512", ".sha1", ".md5", ".sig", ".asc", ".txt", ".blockmap",
    ".yml", ".yaml", ".json", ".minisig", ".sha256sum", ".sha512sum", ".pem", ".sigstore", ".bundle",
)
HELPER_MARKER = re.compile(r"\.(?:sbom|intoto)", re.IGNORECASE)
FILE_TYPES = (
    "tar.gz", "tar.xz", "tar.bz2", "tar.zst", "tar", "tgz", "txz", "tbz2", "tbz",
    "appimage", "deb", "zip", "rpm", "7z", "snap", "flatpak", "pacman", "exe", "dmg", "msi",
)
FILE_TYPE_ALIASES = {"tgz": "tar.gz", "txz": "tar.xz", "tbz2": "tar.bz2", "tbz": "tar.bz2"}
FOREIGN_SYSTEM_SUFFIXES = (".exe", ".dmg", ".msi", ".pkg", ".apk")


def token(*alternatives: str) -> re.Pattern[str]:
    return re.compile(rf"(?<![a-z0-9])(?:{'|'.join(alternatives)})(?![a-z0-9])")


ARCHITECTURE_PATTERNS = (
    ("arm64", token("arm64", "aarch64")),
    ("arm", token("arm", r"armv\d+\w*", "armhf", "armel")),
    ("x86", token("i[3-6]86", "linux32", "32-?bit", "ia32", "x32", r"x86(?![_-]?64)")),
    ("x64", token("x86[_-]64", "amd64", "x64", "linux64", "64-?bit")),
)
SYSTEM_PATTERNS = (
    ("windows", token("win", "windows", "win32", "win64")),
    ("mac", token("mac", "macos", "macosx", "osx", "darwin")),
    ("linux", token("linux", "linux32", "linux64")),
)
FOREIGN_SYSTEMS = ("windows", "mac")


def is_helper_file(name: str) -> bool:
    lowered = name.lower()
    return lowered.endswith(HELPER_SUFFIXES) or HELPER_MARKER.search(lowered) is not None


def file_type(name: str) -> str | None:
    lowered = name.lower()
    for extension in FILE_TYPES:
        if lowered.endswith(f".{extension}"):
            return FILE_TYPE_ALIASES.get(extension, extension)
    suffix = lowered.rpartition(".")[2]
    return suffix if "." in lowered and suffix.isalnum() else None


def architecture_family(name: str) -> str | None:
    lowered = name.lower()
    for family, pattern in ARCHITECTURE_PATTERNS:
        if pattern.search(lowered):
            return family
    return None


def system_family(name: str) -> str | None:
    lowered = name.lower()
    if lowered.endswith(FOREIGN_SYSTEM_SUFFIXES):
        return "windows" if lowered.endswith((".exe", ".msi")) else "mac"
    for family, pattern in SYSTEM_PATTERNS:
        if pattern.search(lowered):
            return family
    return None


def matches_architecture(old_name: str, candidate: str) -> bool:
    expected = architecture_family(old_name) or "x64"
    found = architecture_family(candidate)
    return found is None or found == expected


def matches_system(old_name: str, candidate: str) -> bool:
    found = system_family(candidate)
    return found not in FOREIGN_SYSTEMS or found == system_family(old_name)


def matches_file_type(old_name: str, candidate: str) -> bool:
    expected = file_type(old_name)
    return expected is None or file_type(candidate) == expected
