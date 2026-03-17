use std::path::{Component, Path};

/// Safely joins a base path with a relative path, preventing path traversal.
pub(crate) fn safe_join_filesystem_path(base: &str, rel: &str) -> Option<String> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return None;
    }

    for comp in rel_path.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Path::new(base).join(rel_path).to_str().map(|s| s.to_string())
}
