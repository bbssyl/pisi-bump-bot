use std::io;

use roxmltree::Error as XmlError;

#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
    #[error("{path}: dizin bulunamadı")]
    DirectoryNotFound { path: String },
    #[error("{path}: pspec.xml bulunamadı")]
    NoRecipesFound { path: String },
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RecipeReadError {
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("xml parse error in {path}: {source}")]
    Xml {
        path: String,
        #[source]
        source: XmlError,
    },
    #[error("{path}: missing Source/Name")]
    EmptyName { path: String },
}
