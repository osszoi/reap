use crate::java::graph::ModuleGraph;
use crate::types::UnusedFile;
use std::path::Path;

pub fn collect(graph: &ModuleGraph, cwd: &Path) -> Vec<UnusedFile> {
    let mut unused: Vec<UnusedFile> = graph
        .files
        .iter()
        .enumerate()
        .filter(|(id, _)| !graph.reachable[*id] && !graph.is_root[*id])
        .map(|(_, file)| file)
        .filter(|file| !is_excluded(&file.path))
        .map(|file| {
            let rel = file.path.strip_prefix(cwd).unwrap_or(&file.path);
            UnusedFile { path: rel.to_string_lossy().into_owned() }
        })
        .collect();

    unused.sort_by(|a, b| a.path.cmp(&b.path));
    unused
}

fn is_excluded(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    matches!(name.as_ref(), "package-info.java" | "module-info.java")
}
