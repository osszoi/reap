use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Java,
    Unknown,
}

pub struct ProjectInfo {
    pub lang: Lang,
    pub root: PathBuf,
    pub name: String,
}

pub fn detect_project(cwd: &Path) -> ProjectInfo {
    match find_upwards("pom.xml", cwd) {
        Some(root) => {
            let name = root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            ProjectInfo { lang: Lang::Java, root, name }
        }
        None => ProjectInfo { lang: Lang::Unknown, root: cwd.to_path_buf(), name: String::new() },
    }
}

fn find_upwards(filename: &str, from: &Path) -> Option<PathBuf> {
    let mut dir = from;
    for _ in 0..6 {
        if dir.join(filename).exists() {
            return Some(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) if parent != dir => dir = parent,
            _ => break,
        }
    }
    None
}
