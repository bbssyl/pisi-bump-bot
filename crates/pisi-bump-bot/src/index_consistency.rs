use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};

use pisi_bump_common::PackageRecipe;

const INDEX_FILE_NAMES: [&str; 2] = ["pisi-index.xml.xz", "pisi-index.xml"];
const LEADING_NOISE: &[char] = &['\u{feff}', ' ', '\t', '\r', '\n'];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexMismatch {
    pub name: String,
    pub recipe_path: String,
    pub pspec_version: String,
    pub index_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexConsistency {
    pub found: bool,
    pub error: Option<String>,
    pub missing_from_index: Vec<String>,
    pub version_mismatches: Vec<IndexMismatch>,
}

pub fn not_found() -> IndexConsistency {
    IndexConsistency {
        found: false,
        error: None,
        missing_from_index: Vec::new(),
        version_mismatches: Vec::new(),
    }
}

pub fn failed(reason: impl Into<String>) -> IndexConsistency {
    IndexConsistency {
        found: true,
        error: Some(reason.into()),
        missing_from_index: Vec::new(),
        version_mismatches: Vec::new(),
    }
}

pub fn locate_index(root: &Path) -> Option<PathBuf> {
    INDEX_FILE_NAMES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn direct_children<'a, 'input>(
    node: Node<'a, 'input>,
    tag: &'static str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(move |child| child.has_tag_name(tag))
}

fn element_text(node: Node, tag: &'static str) -> Option<String> {
    direct_children(node, tag)
        .next()
        .and_then(|child| child.text())
        .map(|text| text.trim().to_string())
}

fn release_number(update: Node) -> u32 {
    update
        .attribute("release")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn newest_update<'a, 'input>(spec_file: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    let updates: Vec<Node> = direct_children(spec_file, "History")
        .flat_map(|history| direct_children(history, "Update"))
        .collect();
    let highest = updates.iter().map(|update| release_number(*update)).max()?;
    updates
        .into_iter()
        .find(|update| release_number(*update) == highest)
}

fn parse_index_document(text: &str) -> Result<Document<'_>, roxmltree::Error> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    Document::parse_with_options(text.trim_start_matches(LEADING_NOISE), options)
}

fn collect_versions(root: Node) -> BTreeMap<String, String> {
    let mut versions = BTreeMap::new();
    for spec_file in direct_children(root, "SpecFile") {
        let Some(source) = direct_children(spec_file, "Source").next() else {
            continue;
        };
        let name = element_text(source, "Name").filter(|name| !name.is_empty());
        if name.is_none() {
            continue;
        }
        let Some(recipe_path) = element_text(source, "SourceURI").filter(|uri| !uri.is_empty())
        else {
            continue;
        };
        let version = newest_update(spec_file)
            .and_then(|update| element_text(update, "Version"))
            .unwrap_or_default();
        versions.insert(recipe_path, version);
    }
    versions
}

fn decompress_if_needed(path: &Path, bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if path.extension().and_then(|value| value.to_str()) != Some("xz") {
        return Ok(bytes);
    }
    let mut decompressed = Vec::new();
    lzma_rs::xz_decompress(&mut Cursor::new(bytes), &mut decompressed)
        .map_err(|error| error.to_string())?;
    Ok(decompressed)
}

fn read_index_versions(index_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let raw = fs::read(index_path).map_err(|error| error.to_string())?;
    let xml_bytes = decompress_if_needed(index_path, raw)?;
    let text = String::from_utf8(xml_bytes).map_err(|error| error.to_string())?;
    let document = parse_index_document(&text).map_err(|error| error.to_string())?;
    Ok(collect_versions(document.root_element()))
}

pub fn compare_with_index(
    recipes: &[PackageRecipe],
    versions: &BTreeMap<String, String>,
) -> IndexConsistency {
    let mut missing: Vec<String> = recipes
        .iter()
        .filter(|recipe| !versions.contains_key(&recipe.recipe_path))
        .map(|recipe| recipe.recipe_path.clone())
        .collect();
    missing.sort();

    let mut ordered_recipes: Vec<&PackageRecipe> = recipes.iter().collect();
    ordered_recipes.sort_by(|left, right| left.recipe_path.cmp(&right.recipe_path));

    let mismatches = ordered_recipes
        .into_iter()
        .filter_map(|recipe| {
            let index_version = versions.get(&recipe.recipe_path)?;
            if *index_version == recipe.current_version {
                return None;
            }
            Some(IndexMismatch {
                name: recipe.name.clone(),
                recipe_path: recipe.recipe_path.clone(),
                pspec_version: recipe.current_version.clone(),
                index_version: index_version.clone(),
            })
        })
        .collect();

    IndexConsistency {
        found: true,
        error: None,
        missing_from_index: missing,
        version_mismatches: mismatches,
    }
}

pub fn check_index_consistency(root: &Path, recipes: &[PackageRecipe]) -> IndexConsistency {
    let Some(index_path) = locate_index(root) else {
        return not_found();
    };
    match read_index_versions(&index_path) {
        Ok(versions) => compare_with_index(recipes, &versions),
        Err(reason) => failed(format!("index okunamadı: {reason}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn recipe(path: &str, version: &str) -> PackageRecipe {
        PackageRecipe {
            recipe_path: path.to_string(),
            name: "pkg".to_string(),
            current_version: version.to_string(),
            current_release: 1,
            archive_urls: vec!["https://github.com/o/r/archive/v1.tar.gz".to_string()],
        }
    }

    fn spec_file_xml(name: &str, updates: &[(u32, &str)], source_uri: &str) -> String {
        let history: String = updates
            .iter()
            .map(|(release, version)| {
                format!(
                    "<Update release=\"{release}\"><Date>2026-01-01</Date><Version>{version}</Version></Update>"
                )
            })
            .collect();
        format!(
            "<SpecFile><Source><Name>{name}</Name><Archive type=\"targz\" sha1sum=\"x\">https://example.org/a.tar.gz</Archive><SourceURI>{source_uri}</SourceURI></Source><History>{history}</History></SpecFile>"
        )
    }

    fn write_index(root: &Path, entries: &[String], compressed: bool) {
        let body = format!("<PISI>{}</PISI>", entries.join(""));
        if compressed {
            let mut compressed_bytes = Vec::new();
            lzma_rs::xz_compress(&mut Cursor::new(body.as_bytes()), &mut compressed_bytes).unwrap();
            fs::write(root.join("pisi-index.xml.xz"), compressed_bytes).unwrap();
        } else {
            fs::write(root.join("pisi-index.xml"), body).unwrap();
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pisi-bump-bot-index-consistency-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn should_list_missing_recipes_and_version_mismatches() {
        let root = temp_dir("mismatch");
        let entry = spec_file_xml("a", &[(1, "1.0")], "a/pspec.xml");
        write_index(&root, &[entry], true);
        let recipes = [recipe("a/pspec.xml", "2.0"), recipe("b/pspec.xml", "1.0")];
        let result = check_index_consistency(&root, &recipes);
        assert_eq!(result.missing_from_index, vec!["b/pspec.xml".to_string()]);
        assert_eq!(
            (
                result.version_mismatches[0].pspec_version.as_str(),
                result.version_mismatches[0].index_version.as_str()
            ),
            ("2.0", "1.0")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn should_use_highest_release_from_index_history() {
        let root = temp_dir("highest");
        let entry = spec_file_xml("a", &[(1, "1.0"), (2, "2.0")], "a/pspec.xml");
        write_index(&root, &[entry], true);
        let recipes = [recipe("a/pspec.xml", "2.0")];
        let result = check_index_consistency(&root, &recipes);
        assert!(result.version_mismatches.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn should_read_plain_index_when_xz_is_absent() {
        let root = temp_dir("plain");
        let entry = spec_file_xml("a", &[(1, "1.0")], "a/pspec.xml");
        write_index(&root, &[entry], false);
        let recipes = [recipe("a/pspec.xml", "1.0")];
        let result = check_index_consistency(&root, &recipes);
        assert!(result.found);
        assert!(result.missing_from_index.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn should_report_not_found_when_no_index_exists() {
        let root = temp_dir("absent");
        let result = check_index_consistency(&root, &[]);
        assert!(!result.found);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn should_report_error_when_index_is_corrupt() {
        let root = temp_dir("corrupt");
        let mut file = fs::File::create(root.join("pisi-index.xml.xz")).unwrap();
        file.write_all(b"junk").unwrap();
        let result = check_index_consistency(&root, &[]);
        assert!(result.error.is_some());
        let _ = fs::remove_dir_all(&root);
    }
}
