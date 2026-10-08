mod common;

use std::fs;

use pisi_bump_common::{load_recipes, read_recipe};

use common::{TempDir, copy_tree, fixtures_dir};

#[test]
fn should_parse_brave_fixture_with_doctype_and_multiline_archive() {
    let root = fixtures_dir("pspec");
    let recipe = read_recipe(&root, "brave.xml").expect("brave.xml should parse");
    assert_eq!(recipe.name, "brave-browser");
    assert_eq!(recipe.current_version, "1.93.129");
    assert_eq!(recipe.current_release, 3);
    assert_eq!(
        recipe.archive_urls,
        vec![
            "https://github.com/brave/brave-browser/releases/download/v1.93.129/brave-browser-1.93.129-linux-amd64.zip"
                .to_string()
        ]
    );
}

#[test]
fn should_parse_multi_archive_fixture_with_leading_blank_line() {
    let root = fixtures_dir("pspec");
    let recipe = read_recipe(&root, "multi-archive.xml").expect("multi-archive.xml should parse");
    assert_eq!(recipe.name, "brother-1210w");
    assert_eq!(recipe.current_version, "3.0.1");
    assert_eq!(recipe.current_release, 1);
    assert_eq!(
        recipe.archive_urls,
        vec![
            "http://download.brother.com/welcome/dlf101549/hl1210wlpr-3.0.1-1.i386.rpm".to_string(),
            "http://download.brother.com/welcome/dlf101548/hl1210wcupswrapper-3.0.1-1.i386.rpm"
                .to_string(),
        ]
    );
}

#[test]
fn should_parse_odd_indent_fixture_selecting_highest_release_out_of_order() {
    let root = fixtures_dir("pspec");
    let recipe = read_recipe(&root, "odd-indent.xml").expect("odd-indent.xml should parse");
    assert_eq!(recipe.name, "JEdit");
    assert_eq!(recipe.current_version, "5.5.0");
    assert_eq!(recipe.current_release, 2);
}

#[test]
fn should_parse_tabs_fixture_with_tab_characters_in_body() {
    let root = fixtures_dir("pspec");
    let recipe = read_recipe(&root, "tabs.xml").expect("tabs.xml should parse");
    assert_eq!(recipe.name, "opera");
    assert_eq!(recipe.current_version, "133.0.5932.85");
    assert_eq!(recipe.current_release, 54);
}

#[test]
fn should_parse_atari800_fixture_with_single_history_entry() {
    let root = fixtures_dir("pspec");
    let recipe = read_recipe(&root, "atari800.xml").expect("atari800.xml should parse");
    assert_eq!(recipe.name, "atari800");
    assert_eq!(recipe.current_version, "5.2.0");
    assert_eq!(recipe.current_release, 1);
    assert_eq!(
        recipe.archive_urls,
        vec![
            "https://github.com/atari800/atari800/releases/download/ATARI800_5_2_0/atari800-5.2.0-src.tgz"
                .to_string()
        ]
    );
}

#[test]
fn should_load_multi_package_recipes_tree_skipping_excluded_directories() {
    let workspace = TempDir::new();
    let root = workspace.path().join("recipes-tree");
    copy_tree(&fixtures_dir("recipes-tree"), &root);
    let hidden_git_recipe = root.join(".git/internal/pspec.xml");
    fs::create_dir_all(hidden_git_recipe.parent().unwrap()).unwrap();
    fs::copy(
        fixtures_dir("hidden-git-recipe/pspec.xml"),
        &hidden_git_recipe,
    )
    .unwrap();
    let load = load_recipes(&root).expect("recipes-tree should load");
    let mut paths: Vec<String> = load
        .recipes
        .iter()
        .map(|recipe| recipe.recipe_path.clone())
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "editor/jedit/pspec.xml".to_string(),
            "games/emulator/atari800/pspec.xml".to_string(),
            "hardware/printer/brother-1210w/pspec.xml".to_string(),
            "network/browser/brave/pspec.xml".to_string(),
        ]
    );
    assert!(load.unreadable_paths.is_empty());
    let mut names: Vec<String> = load.recipes.into_iter().map(|recipe| recipe.name).collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "JEdit".to_string(),
            "atari800".to_string(),
            "brave-browser".to_string(),
            "brother-1210w".to_string(),
        ]
    );
}
