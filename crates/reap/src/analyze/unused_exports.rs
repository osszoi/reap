use crate::java::model::FileInfo;
use crate::types::UnusedExport;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// Heuristic: a public/protected method whose name is referenced in no *other* file.
// Name-based (ignores overloading/qualification) — conservative to avoid false positives.
pub fn collect(files: &[FileInfo], cwd: &Path) -> Vec<UnusedExport> {
    let mut name_to_files: HashMap<&str, HashSet<usize>> = HashMap::new();
    for (id, file) in files.iter().enumerate() {
        for name in &file.used_names {
            name_to_files.entry(name.as_str()).or_default().insert(id);
        }
    }

    let mut unused: Vec<UnusedExport> = Vec::new();
    for (id, file) in files.iter().enumerate() {
        if is_test_path(&file.path.to_string_lossy()) {
            continue;
        }
        let rel = file.path.strip_prefix(cwd).unwrap_or(&file.path).to_string_lossy().into_owned();
        for member in &file.exports {
            let used_elsewhere = name_to_files
                .get(member.name.as_str())
                .map(|set| set.iter().any(|&fid| fid != id))
                .unwrap_or(false);
            if !used_elsewhere {
                unused.push(UnusedExport { path: rel.clone(), name: member.name.clone(), line: member.line });
            }
        }
    }

    unused.sort_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
    unused
}

fn is_test_path(path: &str) -> bool {
    path.contains("/src/test/") || path.contains("/test/")
}
