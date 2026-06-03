use crate::java::model::FileInfo;
use crate::types::LargeFunction;
use std::path::Path;

const LARGE_THRESHOLD: u32 = 60;

pub fn collect(files: &[FileInfo], cwd: &Path) -> Vec<LargeFunction> {
    let mut entries: Vec<LargeFunction> = files
        .iter()
        .flat_map(|f| {
            let rel = rel_path(&f.path, cwd);
            f.functions
                .iter()
                .filter(|fn_| fn_.line_count > LARGE_THRESHOLD)
                .map(move |fn_| LargeFunction {
                    path: rel.clone(),
                    name: fn_.name.clone(),
                    line: fn_.line,
                    line_count: fn_.line_count,
                })
        })
        .collect();

    entries.sort_by(|a, b| b.line_count.cmp(&a.line_count));
    entries
}

fn rel_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).to_string_lossy().into_owned()
}
