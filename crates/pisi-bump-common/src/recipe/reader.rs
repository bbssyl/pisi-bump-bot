use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{RecipeError, RecipeReadError};
use crate::recipe::spec::{PackageRecipe, parse_recipe};

const PSPEC_FILE_NAME: &str = "pspec.xml";
const EXCLUDED_DIRECTORIES: [&str; 2] = ["0oldpackage", ".git"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeLoad {
    pub recipes: Vec<PackageRecipe>,
    pub unreadable_paths: Vec<String>,
}

fn relative_posix_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn walk(root: &Path, directory: &Path, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut subdirectories: Vec<PathBuf> = Vec::new();
    let mut has_pspec = false;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !EXCLUDED_DIRECTORIES.contains(&name.as_ref()) {
                subdirectories.push(entry.path());
            }
        } else if entry.file_name().to_str() == Some(PSPEC_FILE_NAME) {
            has_pspec = true;
        }
    }
    if has_pspec {
        found.push(relative_posix_path(root, &directory.join(PSPEC_FILE_NAME)));
    }
    for subdirectory in subdirectories {
        walk(root, &subdirectory, found);
    }
}

fn find_pspec_paths(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

fn try_read_recipe(root: &Path, recipe_path: &str) -> Result<PackageRecipe, RecipeReadError> {
    let xml_text =
        fs::read_to_string(root.join(recipe_path)).map_err(|source| RecipeReadError::Io {
            path: recipe_path.to_string(),
            source,
        })?;
    parse_recipe(recipe_path, &xml_text)
}

pub fn read_recipe(root: &Path, recipe_path: &str) -> Option<PackageRecipe> {
    try_read_recipe(root, recipe_path).ok()
}

pub fn load_recipes(root: &Path) -> Result<RecipeLoad, RecipeError> {
    if !root.is_dir() {
        return Err(RecipeError::DirectoryNotFound {
            path: root.display().to_string(),
        });
    }
    let paths = find_pspec_paths(root);
    if paths.is_empty() {
        return Err(RecipeError::NoRecipesFound {
            path: root.display().to_string(),
        });
    }
    let mut recipes = Vec::new();
    let mut unreadable_paths = Vec::new();
    for path in paths {
        match read_recipe(root, &path) {
            Some(recipe) => recipes.push(recipe),
            None => unreadable_paths.push(path),
        }
    }
    Ok(RecipeLoad {
        recipes,
        unreadable_paths,
    })
}
