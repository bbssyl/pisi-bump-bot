use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use pisi_bump_bot::archive_download::DownloadResult;
use pisi_bump_bot::cli::{self, Runtime};
use pisi_bump_bot::error::{ArchiveDownloadError, FetchError};
use pisi_bump_bot::http_client::HttpResponse;

const API: &str = "https://api.github.com/repos/";
const FAKE_SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

fn temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "pisi-bump-bot-cli-integration-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn spec_xml(name: &str, updates: &[(u32, &str)], archive: &str) -> String {
    let history: String = updates
        .iter()
        .map(|(release, version)| {
            format!(
                "<Update release=\"{release}\"><Date>2026-01-01</Date><Version>{version}</Version></Update>"
            )
        })
        .collect();
    format!(
        "<PISI><Source><Name>{name}</Name><Archive type=\"targz\" sha1sum=\"x\">{archive}</Archive></Source><History>{history}</History></PISI>"
    )
}

fn spec_file_entry(name: &str, updates: &[(u32, &str)], archive: &str, source_uri: &str) -> String {
    let history: String = updates
        .iter()
        .map(|(release, version)| {
            format!(
                "<Update release=\"{release}\"><Date>2026-01-01</Date><Version>{version}</Version></Update>"
            )
        })
        .collect();
    format!(
        "<SpecFile><Source><Name>{name}</Name><Archive type=\"targz\" sha1sum=\"x\">{archive}</Archive><SourceURI>{source_uri}</SourceURI></Source><History>{history}</History></SpecFile>"
    )
}

fn write_pspec(root: &Path, recipe_path: &str, content: &str) {
    let target = root.join(recipe_path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, content).unwrap();
}

fn write_index(root: &Path, entries: &[String]) {
    let body = format!("<PISI>{}</PISI>", entries.join(""));
    let mut compressed = Vec::new();
    lzma_rs::xz_compress(&mut Cursor::new(body.as_bytes()), &mut compressed).unwrap();
    fs::write(root.join("pisi-index.xml.xz"), compressed).unwrap();
}

fn build_root(directory: &Path) -> PathBuf {
    let root = directory.join("contrib");
    let archive = "https://github.com/ttcdt/mp-5.x/archive/refs/tags/5.55.tar.gz";
    write_pspec(
        &root,
        "editor/mp/pspec.xml",
        &spec_xml("mp", &[(1, "5.0"), (2, "5.55")], archive),
    );
    write_pspec(
        &root,
        "editor/other/pspec.xml",
        &spec_xml("other", &[(1, "1.0")], "https://example.org/x.tar.gz"),
    );
    write_index(
        &root,
        &[spec_file_entry(
            "mp",
            &[(2, "5.2")],
            archive,
            "editor/mp/pspec.xml",
        )],
    );
    root
}

#[allow(clippy::type_complexity)]
fn build_runtime() -> Runtime {
    let fetch: pisi_bump_bot::github_upstream::Fetch =
        Box::new(move |url: &str, _headers: &[(String, String)]| {
            if url.starts_with(&format!("{API}ttcdt/mp-5.x/releases/latest")) {
                return Ok(HttpResponse {
                    status: 200,
                    headers: HashMap::new(),
                    body: br#"{"tag_name":"6.0"}"#.to_vec(),
                });
            }
            Ok(HttpResponse {
                status: 404,
                headers: HashMap::new(),
                body: Vec::new(),
            })
        });
    let hash_archive: Box<dyn Fn(&str) -> Result<String, FetchError>> =
        Box::new(|_url: &str| Ok("sha".to_string()));
    let download: Box<dyn Fn(&str) -> Result<DownloadResult, ArchiveDownloadError>> =
        Box::new(|url: &str| {
            Ok(DownloadResult {
                url: url.to_string(),
                sha1: FAKE_SHA1.to_string(),
                size_bytes: 10,
            })
        });
    Runtime::new(fetch, hash_archive, None, download)
}

fn run_cli(base: &Path, extra: &[&str]) -> i32 {
    let mut argv = vec![
        "pisi-bump-bot".to_string(),
        "report".to_string(),
        "--recipes-dir".to_string(),
        base.join("contrib").to_string_lossy().into_owned(),
        "--json".to_string(),
        base.join("r.json").to_string_lossy().into_owned(),
        "--markdown".to_string(),
        base.join("r.md").to_string_lossy().into_owned(),
        "--source-commit".to_string(),
        "abc1234".to_string(),
    ];
    argv.extend(extra.iter().map(|value| value.to_string()));
    cli::run(argv, build_runtime())
}

#[test]
fn should_write_reports_and_exit_zero_when_recipes_are_available() {
    let directory = temp_dir("write-reports");
    build_root(&directory);
    let code = run_cli(&directory, &[]);
    let text = fs::read_to_string(directory.join("r.md")).unwrap();
    assert_eq!(code, 0);
    assert!(text.contains("| mp | `editor/mp/pspec.xml` | 5.55 | 6.0 |"));
    assert!(text.contains("| mp | `editor/mp/pspec.xml` | 5.55 | 5.2 |"));
    assert!(text.contains("commit `abc1234`"));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_exit_non_zero_when_recipes_dir_is_missing() {
    let directory = temp_dir("missing-recipes");
    let code = run_cli(&directory, &[]);
    assert_eq!(code, 1);
    assert!(!directory.join("r.json").exists());
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_produce_identical_files_and_empty_updates_when_run_twice() {
    let directory = temp_dir("twice");
    build_root(&directory);
    run_cli(&directory, &[]);
    let first_json = fs::read_to_string(directory.join("r.json")).unwrap();
    let first_md = fs::read_to_string(directory.join("r.md")).unwrap();
    let previous = directory.join("r.json").to_string_lossy().into_owned();
    let new_updates = directory.join("new.md").to_string_lossy().into_owned();
    run_cli(
        &directory,
        &["--previous", &previous, "--new-updates", &new_updates],
    );
    let second_json = fs::read_to_string(directory.join("r.json")).unwrap();
    let second_md = fs::read_to_string(directory.join("r.md")).unwrap();
    let updates = fs::read_to_string(directory.join("new.md")).unwrap();
    assert_eq!(first_json, second_json);
    assert_eq!(first_md, second_md);
    assert_eq!(updates, "");
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_list_update_when_previous_report_had_older_latest() {
    let directory = temp_dir("older-latest");
    build_root(&directory);
    run_cli(&directory, &[]);
    let original = fs::read_to_string(directory.join("r.json")).unwrap();
    let modified = original.replace("\"6.0\"", "\"5.9\"");
    fs::write(directory.join("prev.json"), modified).unwrap();
    let previous = directory.join("prev.json").to_string_lossy().into_owned();
    let new_updates = directory.join("new.md").to_string_lossy().into_owned();
    run_cli(
        &directory,
        &["--previous", &previous, "--new-updates", &new_updates],
    );
    let updates = fs::read_to_string(directory.join("new.md")).unwrap();
    assert!(updates.contains("**mp**"));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_render_markdown_when_render_command_runs() {
    let directory = temp_dir("render");
    build_root(&directory);
    run_cli(&directory, &[]);
    let code = cli::run(
        vec![
            "pisi-bump-bot".to_string(),
            "render".to_string(),
            "--report".to_string(),
            directory.join("r.json").to_string_lossy().into_owned(),
            "--state".to_string(),
            directory.join("s.json").to_string_lossy().into_owned(),
            "--markdown".to_string(),
            directory.join("o.md").to_string_lossy().into_owned(),
            "--relative-files".to_string(),
        ],
        build_runtime(),
    );
    let text = fs::read_to_string(directory.join("o.md")).unwrap();
    assert_eq!(code, 0);
    assert!(text.contains("Hazır pspec"));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_fail_render_when_report_is_missing() {
    let directory = temp_dir("render-missing");
    let code = cli::run(
        vec![
            "pisi-bump-bot".to_string(),
            "render".to_string(),
            "--report".to_string(),
            "/nonexistent.json".to_string(),
            "--state".to_string(),
            "/nonexistent-state.json".to_string(),
            "--markdown".to_string(),
            directory.join("o.md").to_string_lossy().into_owned(),
        ],
        build_runtime(),
    );
    assert_eq!(code, 1);
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_write_state_and_comment_when_merge_command_runs() {
    let directory = temp_dir("merge");
    fs::create_dir_all(&directory).unwrap();
    let state_path = directory.join("s.json");
    let state_json = r#"{"packages":{"alpha":{"version":"v2","status":"hazir","date":"d","reason":null,"run_id":null,"artifact_name":null,"attempts":0,"logic_version":2}}}"#;
    fs::write(&state_path, state_json).unwrap();
    let results_dir = directory.join("res");
    fs::create_dir_all(&results_dir).unwrap();
    fs::write(
        results_dir.join("result-alpha.json"),
        r#"{"recipe_path":"alpha","version":"v2","status":"basarisiz","run_id":"3"}"#,
    )
    .unwrap();
    let code = cli::run(
        vec![
            "pisi-bump-bot".to_string(),
            "merge-results".to_string(),
            "--state".to_string(),
            state_path.to_string_lossy().into_owned(),
            "--results-dir".to_string(),
            results_dir.to_string_lossy().into_owned(),
            "--comment".to_string(),
            directory.join("c.md").to_string_lossy().into_owned(),
            "--repo-web-url".to_string(),
            "https://github.com/o/r".to_string(),
        ],
        build_runtime(),
    );
    let comment = fs::read_to_string(directory.join("c.md")).unwrap();
    let state_text = fs::read_to_string(&state_path).unwrap();
    assert_eq!(code, 0);
    assert!(state_text.contains("\"basarisiz\""));
    assert!(comment.contains("❌"));
    let _ = fs::remove_dir_all(&directory);
}

#[test]
fn should_exit_with_usage_code_when_arguments_are_invalid() {
    let code = cli::run(
        vec!["pisi-bump-bot".to_string(), "not-a-command".to_string()],
        build_runtime(),
    );
    assert_eq!(code, 2);
}
