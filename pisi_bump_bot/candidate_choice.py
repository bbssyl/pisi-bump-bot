from dataclasses import dataclass
from urllib.parse import quote, unquote

from pisi_bump_bot.asset_selector import VersionPair, select_asset
from pisi_bump_bot.candidate_url import build_candidate_url, derive_new_version
from pisi_bump_bot.github_upstream import GithubArchive, LatestRelease, is_release_download
from pisi_bump_bot.spec_parser import PackageRecipe

RELEASE_DOWNLOAD_ROOT = "https://github.com"


@dataclass(frozen=True)
class CandidateChoice:
    url: str | None
    reason: str | None = None


def select_release_asset(
    recipe: PackageRecipe, archive: GithubArchive, release: LatestRelease, new_version: str
) -> CandidateChoice:
    old_name = unquote(recipe.archive_urls[0].rpartition("/")[2])
    selection = select_asset(old_name, VersionPair(recipe.current_version, new_version), release.assets or ())
    if selection.name is None:
        return CandidateChoice(None, selection.reason)
    base = f"{RELEASE_DOWNLOAD_ROOT}/{archive.owner}/{archive.repo}/releases/download/{release.tag}"
    return CandidateChoice(f"{base}/{quote(selection.name)}")


def substitute_candidate(
    recipe: PackageRecipe, archive: GithubArchive, release: LatestRelease, new_version: str
) -> CandidateChoice:
    archive_url = recipe.archive_urls[0]
    candidate = build_candidate_url(archive_url, archive.tag, release.tag, recipe.current_version, new_version)
    return CandidateChoice(None if candidate == archive_url else candidate)


def choose_candidate(recipe: PackageRecipe, archive: GithubArchive, release: LatestRelease) -> CandidateChoice:
    new_version = derive_new_version(release.tag, (archive.repo, recipe.name), recipe.current_version)
    if new_version is None:
        return CandidateChoice(None)
    if release.assets is not None and is_release_download(recipe.archive_urls[0]):
        return select_release_asset(recipe, archive, release, new_version)
    return substitute_candidate(recipe, archive, release, new_version)
