use std::process::Command;

pub fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

pub fn gh_auth_token() -> Option<String> {
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_treat_whitespace_only_string_as_empty() {
        assert_eq!(non_empty(Some("   ".to_string())), None);
    }

    #[test]
    fn should_keep_non_empty_string_unchanged() {
        assert_eq!(
            non_empty(Some("value".to_string())),
            Some("value".to_string())
        );
    }

    #[test]
    fn should_treat_none_as_empty() {
        assert_eq!(non_empty(None), None);
    }
}
