from pisi_bump_bot.candidate_url import build_candidate_url, derive_new_version
from pisi_bump_bot.errors import FetchError, RateLimitExceeded, UpstreamError
from pisi_bump_bot.github_upstream import GithubArchive, GithubLookup, LatestRelease, parse_github_archive
from pisi_bump_bot.http_client import HashArchive
from pisi_bump_bot.spec_parser import PackageRecipe
from pisi_bump_bot.report_model import PackageReport, Status
from pisi_bump_bot.version_compare import compare_versions, normalize_version

UNSUPPORTED_DETAIL = "GitHub dışı veya desteklenmeyen arşiv URL'si"
RATE_LIMIT_DETAIL = "rate limit"


class PackageChecker:
    def __init__(self, lookup: GithubLookup, hash_archive: HashArchive | None) -> None:
        self._lookup = lookup
        self._hash_archive = hash_archive

    def check(self, recipe: PackageRecipe) -> PackageReport:
        archive = parse_github_archive(recipe.archive_urls[0]) if recipe.archive_urls else None
        if archive is None:
            return PackageReport(
                recipe.name, Status.UNSUPPORTED, recipe.current_version,
                detail=UNSUPPORTED_DETAIL, recipe_path=recipe.recipe_path,
            )
        try:
            latest = self._lookup.latest(archive.owner, archive.repo)
        except RateLimitExceeded:
            return self._failure(recipe, archive, RATE_LIMIT_DETAIL)
        except UpstreamError as error:
            return self._failure(recipe, archive, str(error))
        return self._compare(recipe, archive, latest)

    def _failure(self, recipe: PackageRecipe, archive: GithubArchive, reason: str) -> PackageReport:
        upstream = f"{archive.owner}/{archive.repo}"
        return PackageReport(
            recipe.name, Status.ERROR, recipe.current_version,
            upstream=upstream, detail=reason, recipe_path=recipe.recipe_path,
        )

    def _compare(self, recipe: PackageRecipe, archive: GithubArchive, latest: LatestRelease) -> PackageReport:
        prefixes = (archive.repo, recipe.name)
        current = normalize_version(recipe.current_version, prefixes)
        newest = normalize_version(latest.tag, prefixes)
        upstream = f"{archive.owner}/{archive.repo}"
        fields = {
            "name": recipe.name,
            "current_version": recipe.current_version,
            "latest_version": latest.tag,
            "upstream": upstream,
            "release_url": latest.release_url,
            "recipe_path": recipe.recipe_path,
        }
        if current is None or newest is None:
            return PackageReport(status=Status.UNCOMPARABLE, **fields)
        if compare_versions(newest, current) <= 0:
            return PackageReport(status=Status.CURRENT, **fields)
        return self._outdated(recipe, archive, fields)

    def _outdated(self, recipe: PackageRecipe, archive: GithubArchive, fields: dict[str, str]) -> PackageReport:
        candidate = candidate_for(recipe, archive, fields["latest_version"])
        sha1, detail = self._hash(candidate)
        return PackageReport(
            status=Status.OUTDATED, candidate_url=candidate, candidate_sha1=sha1, detail=detail, **fields
        )

    def _hash(self, candidate: str | None) -> tuple[str | None, str | None]:
        if self._hash_archive is None or candidate is None:
            return None, None
        try:
            return self._hash_archive(candidate), None
        except FetchError as error:
            return None, f"sha1 hesaplanamadı: {error}"


def candidate_for(recipe: PackageRecipe, archive: GithubArchive, new_tag: str) -> str | None:
    new_version = derive_new_version(new_tag, (archive.repo, recipe.name), recipe.current_version)
    if new_version is None:
        return None
    archive_url = recipe.archive_urls[0]
    candidate = build_candidate_url(archive_url, archive.tag, new_tag, recipe.current_version, new_version)
    return None if candidate == archive_url else candidate
