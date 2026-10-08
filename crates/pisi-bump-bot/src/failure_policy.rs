use crate::build_state::{BuildEntry, EntryStatus, MAX_ATTEMPTS};

pub const BUILD_LOST_REASON: &str = "derleme sonucu alınamadı";

fn exhausted_prefix() -> String {
    format!("{MAX_ATTEMPTS} denemede de başarısız")
}

pub fn next_attempts(previous: Option<&BuildEntry>, version: &str) -> u32 {
    match previous {
        None => 1,
        Some(entry) if entry.version != version || entry.status == EntryStatus::PrepareFailed => 1,
        Some(entry) => entry.attempts + 1,
    }
}

pub fn transient_outcome(reason: &str, attempts: u32) -> (EntryStatus, String) {
    if attempts < MAX_ATTEMPTS {
        (EntryStatus::TransientFailed, reason.to_string())
    } else {
        (
            EntryStatus::PrepareFailed,
            format!("{}: {reason}", exhausted_prefix()),
        )
    }
}

pub fn failure_outcome(reason: &str, transient: bool, attempts: u32) -> (EntryStatus, String) {
    if transient {
        transient_outcome(reason, attempts)
    } else {
        (EntryStatus::PrepareFailed, reason.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(version: &str, status: EntryStatus, attempts: u32) -> BuildEntry {
        BuildEntry::new(version, status, "2026-10-07").with_attempts(attempts)
    }

    #[test]
    fn should_start_at_one_when_no_previous_entry() {
        assert_eq!(next_attempts(None, "v1"), 1);
    }

    #[test]
    fn should_restart_at_one_when_version_changed() {
        let previous = entry("v1", EntryStatus::TransientFailed, 2);
        assert_eq!(next_attempts(Some(&previous), "v2"), 1);
    }

    #[test]
    fn should_restart_at_one_when_previous_was_prepare_failed() {
        let previous = entry("v1", EntryStatus::PrepareFailed, 1);
        assert_eq!(next_attempts(Some(&previous), "v1"), 1);
    }

    #[test]
    fn should_increment_when_same_version_and_not_prepare_failed() {
        let previous = entry("v1", EntryStatus::TransientFailed, 2);
        assert_eq!(next_attempts(Some(&previous), "v1"), 3);
    }

    #[test]
    fn should_stay_transient_before_max_attempts() {
        let (status, reason) = transient_outcome("zaman aşımı", 2);
        assert_eq!(status, EntryStatus::TransientFailed);
        assert_eq!(reason, "zaman aşımı");
    }

    #[test]
    fn should_become_permanent_at_max_attempts() {
        let (status, reason) = transient_outcome("zaman aşımı", MAX_ATTEMPTS);
        assert_eq!(status, EntryStatus::PrepareFailed);
        assert_eq!(reason, "3 denemede de başarısız: zaman aşımı");
    }

    #[test]
    fn should_map_permanent_failure_directly() {
        let (status, reason) = failure_outcome("HTTP 404", false, 1);
        assert_eq!(status, EntryStatus::PrepareFailed);
        assert_eq!(reason, "HTTP 404");
    }

    #[test]
    fn should_delegate_to_transient_outcome_when_transient() {
        let (status, reason) = failure_outcome("zaman aşımı", true, 1);
        assert_eq!(status, EntryStatus::TransientFailed);
        assert_eq!(reason, "zaman aşımı");
    }
}
