//! Portable paths inside the Mammoth namespace.
use crate::{Error, Result};
use std::path::Path;
/// Normalize a UTF-8 namespace path while rejecting parent traversal.
pub fn normalize(path: &Path) -> Result<String> {
    let raw = path.to_str().ok_or_else(|| Error::InvalidInput("path must be UTF-8".into()))?;
    // PathBuf::join uses native separators for SDK callers on Windows.
    #[cfg(windows)]
    let raw = raw.replace('\\', "/");
    if raw.contains(['\0', '\\']) || raw.split('/').any(|s| s == "..") {
        return Err(Error::InvalidInput(
            "parent traversal, NULs and backslashes are not allowed".into(),
        ));
    }
    let parts: Vec<_> = raw.split('/').filter(|s| !s.is_empty() && *s != ".").collect();
    Ok(format!("/{}", parts.join("/")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_joined_paths_use_the_same_namespace_on_every_platform() {
        let path = Path::new("/team").join("notes").join("memory.txt");
        assert_eq!(normalize(&path).unwrap(), "/team/notes/memory.txt");
        assert!(normalize(&Path::new("/team").join("..").join("secret")).is_err());
        assert!(normalize(Path::new("a\0b")).is_err());
    }
}
