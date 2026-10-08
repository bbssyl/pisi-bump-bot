use std::process::Command;

pub fn resolve_token(cli_token: Option<String>) -> Option<String> {
    if let Some(token) = non_empty(cli_token) {
        return Some(token);
    }
    if let Some(token) = non_empty(std::env::var("GITHUB_TOKEN").ok()) {
        return Some(token);
    }
    gh_auth_token()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

fn gh_auth_token() -> Option<String> {
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    non_empty(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
    ))
}
