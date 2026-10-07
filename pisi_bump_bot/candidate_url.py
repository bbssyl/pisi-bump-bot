import re

SEPARATORS = ("_", "-")
NOT_AFTER_TOKEN = r"(?!\d|\.\d)"
NOT_BEFORE_TOKEN = r"(?<![A-Za-z0-9])"


def version_variants(version: str) -> list[str]:
    return [version] + [version.replace(".", separator) for separator in SEPARATORS if "." in version]


def replace_token(text: str, old_token: str, new_token: str) -> str:
    if not old_token or old_token == new_token:
        return text
    pattern = f"{NOT_BEFORE_TOKEN}{re.escape(old_token)}{NOT_AFTER_TOKEN}"
    return re.sub(pattern, lambda _match: new_token, text)


def replace_version(text: str, old_version: str, new_version: str) -> str:
    for old_variant, new_variant in zip(version_variants(old_version), version_variants(new_version)):
        text = replace_token(text, old_variant, new_variant)
    return text


def build_candidate_url(
    archive_url: str, old_tag: str, new_tag: str, old_version: str, new_version: str
) -> str:
    retagged = replace_token(archive_url, old_tag, new_tag)
    directory, separator, file_name = retagged.rpartition("/")
    return f"{directory}{separator}{replace_version(file_name, old_version, new_version)}"
