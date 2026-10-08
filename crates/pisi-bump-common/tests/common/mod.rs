#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub const ARCHIVE: &str = "https://github.com/o/r/archive/v1.tar.gz";

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let unique = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "pisi-bump-common-recipe-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn fixtures_dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(relative)
}

pub fn spec_xml(name: &str, updates: &[(u32, &str)], archive: &str) -> String {
    let history: String = updates
        .iter()
        .map(|(release, version)| {
            format!(
                "<Update release=\"{release}\"><Date>2026-01-01</Date><Version>{version}</Version></Update>"
            )
        })
        .collect();
    format!(
        "<Source><Name>{name}</Name><Archive type=\"targz\" sha1sum=\"x\">{archive}</Archive></Source><History>{history}</History>"
    )
}

pub fn write_pspec(root: &Path, recipe_path: &str, content: &str) {
    let target = root.join(recipe_path);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(target, format!("<PISI>{content}</PISI>")).unwrap();
}

pub fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
