from dataclasses import dataclass, field

from pisi_bump_bot.build_state import MAX_ATTEMPTS, BuildEntry, EntryStatus, State, package_dir

PENDING_TEXT = "bekliyor"
NOT_PREPARED_PREFIX = "otomatik hazırlanamadı"
FILE_BRANCH = "HEAD"
TRANSIENT_PREFIX = "geçici hata, tekrar denenecek"
MARKS = {EntryStatus.BUILT: "✅", EntryStatus.BUILD_FAILED: "❌", EntryStatus.TRANSIENT_FAILED: "⏳"}
NO_DIFF_STATUSES = (EntryStatus.PREPARE_FAILED, EntryStatus.TRANSIENT_FAILED)


@dataclass(frozen=True)
class BuildLinks:
    state: State = field(default_factory=dict)
    web_url: str | None = None
    relative_files: bool = False

    def entry_for(self, recipe_path: str, version: str | None) -> BuildEntry | None:
        entry = self.state.get(package_dir(recipe_path))
        return entry if entry is not None and entry.version == version else None

    def diff_url(self, recipe_path: str) -> str:
        path = f"hazir/{package_dir(recipe_path)}/pspec.diff"
        if self.relative_files or not self.web_url:
            return path
        return f"{self.web_url}/blob/{FILE_BRANCH}/{path}"

    def run_url(self, entry: BuildEntry) -> str | None:
        if not self.web_url or not entry.run_id:
            return None
        return f"{self.web_url}/actions/runs/{entry.run_id}"

    def prepared_cell(self, recipe_path: str, version: str | None) -> str | None:
        entry = self.entry_for(recipe_path, version)
        if entry is None or entry.status in NO_DIFF_STATUSES:
            return None
        return f"[pspec.diff]({self.diff_url(recipe_path)})"

    def build_cell(self, recipe_path: str, version: str | None) -> str | None:
        entry = self.entry_for(recipe_path, version)
        if entry is None:
            return None
        if entry.status is EntryStatus.PREPARE_FAILED:
            return f"{NOT_PREPARED_PREFIX}: {entry.reason}"
        if entry.status is EntryStatus.TRANSIENT_FAILED:
            return f"{TRANSIENT_PREFIX} ({entry.attempts}/{MAX_ATTEMPTS}): {entry.reason}"
        if entry.status is EntryStatus.PREPARED:
            return PENDING_TEXT
        return result_cell(entry, self.run_url(entry))


def result_cell(entry: BuildEntry, url: str | None) -> str:
    mark = MARKS[entry.status]
    return f"[{mark}]({url})" if url else mark
