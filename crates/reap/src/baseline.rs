use crate::java::extract::FunctionMetrics;
use crate::java::graph::ModuleGraph;
use crate::java::model::{parse_file, FileInfo};
use crate::report_filter::ReportFilter;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub enum BaseFile<'a> {
    Unchanged,
    // added on this branch, renamed, or unreadable at the merge-base
    New,
    Changed(&'a FileInfo),
}

// merge-base versions of the java files a branch changed, so PR mode can tell
// "introduced" apart from "was already there"
pub struct Baseline {
    base: HashMap<String, Option<FileInfo>>,
    // merge-base text of every java file the branch modified or deleted
    sources: HashMap<String, String>,
    head_functions: HashMap<String, Vec<FunctionMetrics>>,
}

impl Baseline {
    pub fn load(repo_root: &Path, filter: &ReportFilter, graph: Option<&ModuleGraph>) -> Option<Self> {
        let merge_base = filter.merge_base()?;
        let mut paths: Vec<&str> = filter.changed_paths();
        paths.extend(filter.base_paths());
        paths.retain(|p| p.ends_with(".java"));
        paths.sort_unstable();
        paths.dedup();
        let loaded: Vec<(String, Option<String>)> = paths
            .par_iter()
            .map(|&path| {
                let source = if filter.is_added(path) { None } else { git_show(repo_root, merge_base, path) };
                (path.to_string(), source)
            })
            .collect();
        let base = loaded
            .par_iter()
            .map(|(path, src)| (path.clone(), src.as_ref().and_then(|s| parse_file(repo_root.join(path), s))))
            .collect();
        let sources = loaded.into_iter().filter_map(|(path, src)| Some((path, src?))).collect();
        let head_functions = graph
            .map(|g| {
                g.files
                    .iter()
                    .filter_map(|f| {
                        let rel = f.path.strip_prefix(repo_root).unwrap_or(&f.path).to_string_lossy().into_owned();
                        paths.contains(&rel.as_str()).then(|| (rel, f.functions.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Baseline { base, sources, head_functions })
    }

    pub fn sources(&self) -> impl Iterator<Item = (&str, &str)> {
        self.sources.iter().map(|(p, s)| (p.as_str(), s.as_str()))
    }

    pub fn file(&self, path: &str) -> BaseFile<'_> {
        match self.base.get(path) {
            None => BaseFile::Unchanged,
            Some(None) => BaseFile::New,
            Some(Some(info)) => BaseFile::Changed(info),
        }
    }

    // `over` returns a bitmask of exceeded metrics; a function is blamed only for a metric
    // it exceeds now but didn't exceed at the merge-base (or when it's new)
    pub fn crossed(&self, path: &str, name: &str, line: u32, over: impl Fn(&FunctionMetrics) -> u8) -> bool {
        let base = match self.file(path) {
            BaseFile::Unchanged => return false,
            BaseFile::New => return true,
            BaseFile::Changed(base) => base,
        };
        let head = self.head_functions.get(path).map(|v| v.as_slice()).unwrap_or_default();
        let Some(f) = head.iter().find(|f| f.line == line && f.name == name) else { return true };
        match twin(base, head, f) {
            Some(old) => over(f) & !over(old) != 0,
            None => true,
        }
    }
}

// same name + arity, same position among its overloads
fn twin<'a>(base: &'a FileInfo, head: &[FunctionMetrics], f: &FunctionMetrics) -> Option<&'a FunctionMetrics> {
    let same = |g: &&FunctionMetrics| g.name == f.name && g.param_count == f.param_count;
    let nth = head.iter().filter(same).position(|g| g.line == f.line)?;
    base.functions.iter().filter(same).nth(nth)
}

fn git_show(repo_root: &Path, rev: &str, path: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["show", &format!("{rev}:{path}")])
        .current_dir(repo_root)
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
impl Baseline {
    pub fn from_sources(base: &[(&str, Option<&str>)], head: &[(&str, &str)]) -> Self {
        let parse = |path: &str, src: &str| parse_file(Path::new("/r").join(path), src).unwrap();
        Baseline {
            base: base.iter().map(|(p, src)| (p.to_string(), src.map(|s| parse(p, s)))).collect(),
            sources: base.iter().filter_map(|(p, src)| Some((p.to_string(), (*src)?.to_string()))).collect(),
            head_functions: head.iter().map(|(p, src)| (p.to_string(), parse(p, src).functions)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn method(name: &str, ifs: usize) -> String {
        let body: String = (0..ifs).map(|i| format!("    if (x == {i}) {{ y++; }}\n")).collect();
        format!("  int {name}(int x) {{\n    int y = 0;\n{body}    return y;\n  }}\n")
    }

    fn class(methods: &[String]) -> String {
        format!("class A {{\n{}}}\n", methods.concat())
    }

    fn over_cyc(f: &FunctionMetrics) -> u8 {
        (f.cyclomatic > 20) as u8 | ((f.cognitive > 15) as u8) << 1
    }

    #[test]
    fn already_over_and_worse_is_not_blamed() {
        let base = class(&[method("legacy", 25)]);
        let head = class(&[method("legacy", 30)]);
        let b = Baseline::from_sources(&[("A.java", Some(&base))], &[("A.java", &head)]);
        assert!(!b.crossed("A.java", "legacy", 2, over_cyc));
    }

    #[test]
    fn pushed_over_is_blamed() {
        let base = class(&[method("medium", 10)]);
        let head = class(&[method("medium", 25)]);
        let b = Baseline::from_sources(&[("A.java", Some(&base))], &[("A.java", &head)]);
        assert!(b.crossed("A.java", "medium", 2, over_cyc));
    }

    #[test]
    fn crossing_a_second_metric_is_blamed() {
        let b = Baseline::from_sources(&[("A.java", Some(&class(&[method("f", 18)])))], &[("A.java", &class(&[method("f", 25)]))]);
        // 18 ifs: cognitive 18 > 15 already, cyclomatic 19 <= 20; 25 ifs crosses cyclomatic
        assert!(b.crossed("A.java", "f", 2, over_cyc));
        assert!(!b.crossed("A.java", "f", 2, |f| (f.cognitive > 15) as u8));
    }

    #[test]
    fn new_function_and_new_file_are_blamed() {
        let base = class(&[method("legacy", 25)]);
        let head = class(&[method("legacy", 25), method("fresh", 25)]);
        let fresh_line = 2 + 30;
        let b = Baseline::from_sources(&[("A.java", Some(&base)), ("N.java", None)], &[("A.java", &head), ("N.java", &head)]);
        assert!(!b.crossed("A.java", "legacy", 2, over_cyc));
        assert!(b.crossed("A.java", "fresh", fresh_line, over_cyc));
        assert!(b.crossed("N.java", "legacy", 2, over_cyc));
        assert!(!b.crossed("Untouched.java", "legacy", 2, over_cyc));
    }

    #[test]
    fn overloads_match_by_position() {
        let two = |a: usize, b: usize| class(&[method("run", a), method("run", b)]);
        let b = Baseline::from_sources(&[("A.java", Some(&two(25, 5)))], &[("A.java", &two(25, 25))]);
        assert!(!b.crossed("A.java", "run", 2, over_cyc));
        assert!(b.crossed("A.java", "run", 2 + 30, over_cyc));
    }
}
