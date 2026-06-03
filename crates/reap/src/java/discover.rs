use crate::java::model::{parse_file, FileInfo};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &["target", "build", "out", ".git", "node_modules", "generated"];

pub fn java_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            !e.file_type().is_dir()
                || !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref())
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| p.extension().is_some_and(|x| x == "java"))
        .collect()
}

pub fn analyze_files(root: &Path) -> Vec<FileInfo> {
    java_files(root)
        .par_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(path).ok()?;
            parse_file(path.clone(), &source)
        })
        .collect()
}
