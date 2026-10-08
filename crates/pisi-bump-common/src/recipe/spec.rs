use roxmltree::{Document, Node, ParsingOptions};

use crate::error::RecipeReadError;

const LEADING_NOISE: &[char] = &['\u{feff}', ' ', '\t', '\r', '\n'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRecipe {
    pub recipe_path: String,
    pub name: String,
    pub current_version: String,
    pub current_release: u32,
    pub archive_urls: Vec<String>,
}

pub(crate) fn parse_release(value: Option<&str>) -> u32 {
    value.and_then(|text| text.parse::<u32>().ok()).unwrap_or(0)
}

pub(crate) fn release_number(update: Node) -> u32 {
    parse_release(update.attribute("release"))
}

fn direct_children<'a, 'input>(
    node: Node<'a, 'input>,
    tag: &'static str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(move |child| child.has_tag_name(tag))
}

fn history_updates<'a, 'input>(root: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    direct_children(root, "History")
        .flat_map(|history| direct_children(history, "Update"))
        .collect()
}

fn source_archives<'a, 'input>(root: Node<'a, 'input>) -> Vec<Node<'a, 'input>> {
    direct_children(root, "Source")
        .flat_map(|source| direct_children(source, "Archive"))
        .collect()
}

pub(crate) fn newest_update<'a, 'input>(root: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    let updates = history_updates(root);
    let highest = updates.iter().map(|update| release_number(*update)).max()?;
    updates
        .into_iter()
        .find(|update| release_number(*update) == highest)
}

fn element_text(node: Node, tag: &'static str) -> Option<String> {
    direct_children(node, tag)
        .next()
        .and_then(|child| child.text())
        .map(|text| text.trim().to_string())
}

fn source_name(root: Node) -> Option<String> {
    let source = direct_children(root, "Source").next()?;
    let name = element_text(source, "Name")?;
    if name.is_empty() { None } else { Some(name) }
}

fn archive_urls(root: Node) -> Vec<String> {
    source_archives(root)
        .into_iter()
        .filter_map(|archive| archive.text())
        .map(|text| text.trim().to_string())
        .collect()
}

fn parse_document(text: &str) -> Result<Document<'_>, roxmltree::Error> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    Document::parse_with_options(text.trim_start_matches(LEADING_NOISE), options)
}

pub(crate) fn parse_recipe(
    recipe_path: &str,
    xml_text: &str,
) -> Result<PackageRecipe, RecipeReadError> {
    let document = parse_document(xml_text).map_err(|source| RecipeReadError::Xml {
        path: recipe_path.to_string(),
        source,
    })?;
    let root = document.root_element();
    let name = source_name(root).ok_or_else(|| RecipeReadError::EmptyName {
        path: recipe_path.to_string(),
    })?;
    let update = newest_update(root);
    let current_version = update
        .and_then(|node| element_text(node, "Version"))
        .unwrap_or_default();
    let current_release = update.map(release_number).unwrap_or(0);
    Ok(PackageRecipe {
        recipe_path: recipe_path.to_string(),
        name,
        current_version,
        current_release,
        archive_urls: archive_urls(root),
    })
}
