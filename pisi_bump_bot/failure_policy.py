from pisi_bump_bot.build_state import MAX_ATTEMPTS, BuildEntry, EntryStatus

EXHAUSTED_PREFIX = f"{MAX_ATTEMPTS} denemede de başarısız"
BUILD_LOST_REASON = "derleme sonucu alınamadı"


def next_attempts(previous: BuildEntry | None, version: str) -> int:
    if previous is None or previous.version != version or previous.status is EntryStatus.PREPARE_FAILED:
        return 1
    return previous.attempts + 1


def transient_outcome(reason: str, attempts: int) -> tuple[EntryStatus, str]:
    if attempts < MAX_ATTEMPTS:
        return EntryStatus.TRANSIENT_FAILED, reason
    return EntryStatus.PREPARE_FAILED, f"{EXHAUSTED_PREFIX}: {reason}"


def failure_outcome(reason: str, transient: bool, attempts: int) -> tuple[EntryStatus, str]:
    if transient:
        return transient_outcome(reason, attempts)
    return EntryStatus.PREPARE_FAILED, reason
