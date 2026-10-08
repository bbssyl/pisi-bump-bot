mod common;

use std::fs;
use std::path::Path;

use common::{ARCHIVE, TempDir, spec_xml, write_pspec};
use pisi_bump_common::{PackageRecipe, RecipeError, load_recipes};

fn load_single(updates: &[(u32, &str)]) -> PackageRecipe {
    let directory = TempDir::new();
    write_pspec(
        directory.path(),
        "a/pkg/pspec.xml",
        &spec_xml("pkg", updates, ARCHIVE),
    );
    load_recipes(directory.path()).unwrap().recipes[0].clone()
}

#[test]
fn should_pick_highest_release_when_it_is_not_first() {
    let recipe = load_single(&[(1, "1.0"), (3, "3.0"), (2, "2.0")]);
    assert_eq!(
        (recipe.current_version.as_str(), recipe.current_release),
        ("3.0", 3)
    );
}

#[test]
fn should_pick_first_listed_when_newest_is_first() {
    let recipe = load_single(&[(2, "2.0"), (1, "1.0")]);
    assert_eq!(recipe.current_version, "2.0");
}

#[test]
fn should_return_empty_version_when_history_is_missing() {
    let recipe = load_single(&[]);
    assert_eq!(recipe.current_version, "");
}

#[test]
fn should_read_archive_urls_from_source() {
    let recipe = load_single(&[(1, "1.0")]);
    assert_eq!(recipe.archive_urls, vec![ARCHIVE.to_string()]);
}

#[test]
fn should_skip_oldpackage_and_git_directories() {
    let directory = TempDir::new();
    let root = directory.path();
    for path in ["x/pspec.xml", "0oldpackage/y/pspec.xml", ".git/z/pspec.xml"] {
        write_pspec(root, path, &spec_xml("p", &[(1, "1")], ARCHIVE));
    }
    let paths: Vec<String> = load_recipes(root)
        .unwrap()
        .recipes
        .into_iter()
        .map(|recipe| recipe.recipe_path)
        .collect();
    assert_eq!(paths, vec!["x/pspec.xml".to_string()]);
}

#[test]
fn should_keep_duplicate_names_as_separate_recipes() {
    let directory = TempDir::new();
    let root = directory.path();
    write_pspec(
        root,
        "util/admin/ventoy/pspec.xml",
        &spec_xml("ventoy", &[(2, "1.1")], ARCHIVE),
    );
    write_pspec(
        root,
        "hardware/disk/ventoy/pspec.xml",
        &spec_xml("ventoy", &[(1, "1.0")], ARCHIVE),
    );
    let mut paths: Vec<String> = load_recipes(root)
        .unwrap()
        .recipes
        .into_iter()
        .map(|recipe| recipe.recipe_path)
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "hardware/disk/ventoy/pspec.xml".to_string(),
            "util/admin/ventoy/pspec.xml".to_string(),
        ]
    );
}

#[test]
fn should_report_unreadable_path_when_xml_is_broken() {
    let directory = TempDir::new();
    let root = directory.path();
    write_pspec(root, "ok/pspec.xml", &spec_xml("ok", &[(1, "1")], ARCHIVE));
    fs::create_dir_all(root.join("bad")).unwrap();
    fs::write(root.join("bad/pspec.xml"), "<PISI").unwrap();
    let load = load_recipes(root).unwrap();
    assert_eq!(load.recipes.len(), 1);
    assert_eq!(load.unreadable_paths, vec!["bad/pspec.xml".to_string()]);
}

#[test]
fn should_read_recipe_when_file_starts_with_blank_line_and_bom() {
    let directory = TempDir::new();
    let root = directory.path();
    fs::create_dir_all(root.join("a")).unwrap();
    let body = format!(
        "<?xml version=\"1.0\"?><PISI>{}</PISI>",
        spec_xml("a", &[(1, "1")], ARCHIVE)
    );
    let mut bytes = vec![0xEFu8, 0xBB, 0xBF, b'\n'];
    bytes.extend_from_slice(body.as_bytes());
    fs::write(root.join("a/pspec.xml"), bytes).unwrap();
    let load = load_recipes(root).unwrap();
    assert_eq!(load.recipes.len(), 1);
}

#[test]
fn should_raise_when_directory_is_missing() {
    let result = load_recipes(Path::new("/nonexistent/pisi-bump-bot-contrib"));
    assert!(matches!(result, Err(RecipeError::DirectoryNotFound { .. })));
}

#[test]
fn should_raise_when_no_pspec_exists() {
    let directory = TempDir::new();
    let result = load_recipes(directory.path());
    assert!(matches!(result, Err(RecipeError::NoRecipesFound { .. })));
}
