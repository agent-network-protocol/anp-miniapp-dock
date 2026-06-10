use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentPackage {
    pub root: PathBuf,
    pub js: Option<String>,
    pub wxml: String,
    pub wxss: Option<String>,
    pub json: Option<String>,
}

impl ComponentPackage {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ComponentLoadError> {
        let root = root.as_ref().to_path_buf();
        let wxml_path = root.join("index.wxml");
        let wxml =
            fs::read_to_string(&wxml_path).map_err(|source| ComponentLoadError::ReadFailed {
                path: wxml_path,
                source,
            })?;

        Ok(Self {
            js: read_optional(root.join("index.js"))?,
            wxss: read_optional(root.join("index.wxss"))?,
            json: read_optional(root.join("index.json"))?,
            root,
            wxml,
        })
    }
}

#[derive(Debug)]
pub enum ComponentLoadError {
    ReadFailed {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for ComponentLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read component file `{}`: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for ComponentLoadError {}

fn read_optional(path: PathBuf) -> Result<Option<String>, ComponentLoadError> {
    match fs::read_to_string(&path) {
        Ok(value) => Ok(Some(value)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ComponentLoadError::ReadFailed { path, source }),
    }
}
