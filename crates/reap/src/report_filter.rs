use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ChangedSet {
    ranges: HashMap<String, Vec<(u32, u32)>>,
    added_files: HashSet<String>,
    pom_changed: bool,
}

pub struct ReportFilter {
    skip: Option<GlobSet>,
    changed: Option<ChangedSet>,
}

impl ReportFilter {
    pub fn new(skip_patterns: &[String], changed: Option<ChangedSet>) -> Self {
        ReportFilter { skip: build_skip(skip_patterns), changed }
    }

    pub fn compare_mode(&self) -> bool {
        self.changed.is_some()
    }

    pub fn skipped(&self, path: &str) -> bool {
        self.skip.as_ref().map(|s| s.is_match(path)).unwrap_or(false)
    }

    fn overlaps(&self, path: &str, start: u32, end: u32) -> bool {
        match &self.changed {
            None => true,
            Some(c) => c
                .ranges
                .get(path)
                .map(|rs| rs.iter().any(|&(s, e)| s <= end && start <= e))
                .unwrap_or(false),
        }
    }

    // sections with a known line span: complexity, large functions, unused exports
    pub fn span_shown(&self, path: &str, start: u32, end: u32) -> bool {
        !self.skipped(path) && self.overlaps(path, start, end)
    }

    // whole-file sections (refactoring targets): changed iff the file has any changed range
    pub fn file_shown(&self, path: &str) -> bool {
        !self.skipped(path)
            && match &self.changed {
                None => true,
                Some(c) => c.ranges.contains_key(path),
            }
    }

    // unused files: only blame files the PR actually added
    pub fn added_shown(&self, path: &str) -> bool {
        !self.skipped(path)
            && match &self.changed {
                None => true,
                Some(c) => c.added_files.contains(path),
            }
    }

    // multi-member sections without line spans (cycles, clone families):
    // keep unless every member is skipped; in compare mode keep if any member changed.
    pub fn multi_shown(&self, paths: &[String]) -> bool {
        let any_unskipped = paths.iter().any(|p| !self.skipped(p));
        let any_changed = match &self.changed {
            None => true,
            Some(c) => paths.iter().any(|p| c.ranges.contains_key(p.as_str())),
        };
        any_unskipped && any_changed
    }

    // clone groups: members carry line spans, so use line-level overlap.
    pub fn group_shown(&self, members: &[(String, u32, u32)]) -> bool {
        let any_unskipped = members.iter().any(|(p, _, _)| !self.skipped(p));
        let any_changed = match &self.changed {
            None => true,
            Some(c) => members.iter().any(|(p, s, e)| {
                c.ranges
                    .get(p.as_str())
                    .map(|rs| rs.iter().any(|&(rs2, re)| rs2 <= *e && *s <= re))
                    .unwrap_or(false)
            }),
        };
        any_unskipped && any_changed
    }

    // hotspots are omitted entirely under compare mode (churn ranking is meaningless for one PR)
    pub fn hotspots_omitted(&self) -> bool {
        self.changed.is_some()
    }

    // dependency issues count as "introduced" only when the PR edited a pom.xml
    pub fn deps_shown(&self) -> bool {
        match &self.changed {
            None => true,
            Some(c) => c.pom_changed,
        }
    }
}

// --- skip-pattern globs (Feature A) ---

fn build_skip(patterns: &[String]) -> Option<GlobSet> {
    let entries: Vec<&str> = patterns.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if entries.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for entry in entries {
        for g in expand_pattern(entry) {
            if let Ok(glob) = GlobBuilder::new(&g).literal_separator(true).build() {
                builder.add(glob);
            }
        }
    }
    builder.build().ok()
}

fn expand_pattern(p: &str) -> Vec<String> {
    let has_meta = p.contains(['*', '?', '[', '{']) || p.starts_with('/');
    if has_meta {
        return vec![p.trim_start_matches('/').to_string()];
    }
    let q = p.trim_matches('/');
    vec![q.to_string(), format!("{q}/**"), format!("**/{q}/**"), format!("**/{q}")]
}

// --- repo root + changed-hunk resolution (Feature B) ---

pub fn repo_root(dir: &Path, fallback: &Path) -> PathBuf {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output();
    if let Ok(o) = out {
        if o.status.success() {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    fallback.to_path_buf()
}

fn validate_ref(reference: &str) -> Result<(), String> {
    if reference.is_empty() {
        return Err("empty ref".into());
    }
    if reference.starts_with('-') {
        return Err(format!("invalid ref '{reference}' (leading dash)"));
    }
    let ok = reference
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._/@{}~^- ".contains(c));
    if !ok {
        return Err(format!("invalid ref '{reference}' (disallowed characters)"));
    }
    Ok(())
}

pub enum ChangedError {
    NotARepo,
    BadRef(String),
}

pub fn build_changed(repo_root: &Path, reference: &str) -> Result<ChangedSet, ChangedError> {
    validate_ref(reference).map_err(ChangedError::BadRef)?;

    if !git_ok(repo_root, &["rev-parse", "--is-inside-work-tree"]) {
        return Err(ChangedError::NotARepo);
    }
    if !git_ok(repo_root, &["rev-parse", "--verify", "--quiet", &format!("{reference}^{{commit}}")]) {
        return Err(ChangedError::BadRef(format!("ref '{reference}' not found")));
    }

    let mut ranges: HashMap<String, Vec<(u32, u32)>> = HashMap::new();
    let mut added: HashSet<String> = HashSet::new();

    let range = format!("{reference}...HEAD");
    if let Some(out) = git_stdout(repo_root, &["diff", "--unified=0", "--no-color", &range]) {
        parse_diff(&out, &mut ranges, &mut added);
    } else {
        return Err(ChangedError::BadRef(format!("cannot diff against '{reference}' (no merge-base?)")));
    }

    if let Some(out) = git_stdout(repo_root, &["diff", "--unified=0", "--no-color", "HEAD"]) {
        parse_diff(&out, &mut ranges, &mut added);
    }

    if let Some(out) = git_stdout(repo_root, &["ls-files", "--full-name", "--others", "--exclude-standard"]) {
        for line in out.lines() {
            let path = line.trim();
            if path.is_empty() {
                continue;
            }
            added.insert(path.to_string());
            ranges.entry(path.to_string()).or_default().push((1, u32::MAX));
        }
    }

    let pom_changed = ranges.keys().chain(added.iter()).any(|p| p.ends_with("pom.xml"));

    Ok(ChangedSet { ranges, added_files: added, pom_changed })
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_stdout(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).current_dir(dir).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn parse_diff(
    out: &str,
    ranges: &mut HashMap<String, Vec<(u32, u32)>>,
    added: &mut HashSet<String>,
) {
    let mut current: Option<String> = None;
    let mut is_new = false;
    for line in out.lines() {
        if line.starts_with("diff --git") {
            current = None;
            is_new = false;
        } else if line.starts_with("new file mode") {
            is_new = true;
        } else if let Some(p) = line.strip_prefix("+++ ") {
            if p == "/dev/null" {
                current = None;
            } else {
                let path = p.strip_prefix("b/").unwrap_or(p).to_string();
                if is_new {
                    added.insert(path.clone());
                }
                current = Some(path);
            }
        } else if line.starts_with("@@") {
            if let (Some(path), Some((start, count))) = (&current, parse_hunk(line)) {
                if count > 0 {
                    ranges.entry(path.clone()).or_default().push((start, start + count - 1));
                }
            }
        }
    }
}

fn parse_hunk(line: &str) -> Option<(u32, u32)> {
    let plus = line.split('+').nth(1)?;
    let token = plus.split_whitespace().next()?;
    let mut parts = token.split(',');
    let start: u32 = parts.next()?.parse().ok()?;
    let count: u32 = match parts.next() {
        Some(c) => c.parse().ok()?,
        None => 1,
    };
    Some((start, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_bare_name_to_match_anywhere() {
        let set = build_skip(&["generated".to_string()]).unwrap();
        assert!(set.is_match("generated/Foo.java"));
        assert!(set.is_match("a/b/generated/Foo.java"));
        assert!(set.is_match("a/b/generated"));
        assert!(!set.is_match("src/main/Foo.java"));
    }

    #[test]
    fn respects_explicit_glob() {
        let set = build_skip(&["**/*.generated.java".to_string()]).unwrap();
        assert!(set.is_match("a/b/Foo.generated.java"));
        assert!(!set.is_match("a/b/Foo.java"));
    }

    #[test]
    fn star_does_not_cross_slash() {
        let set = build_skip(&["src/*".to_string()]).unwrap();
        assert!(set.is_match("src/Foo.java"));
        assert!(!set.is_match("src/sub/Foo.java"));
    }

    #[test]
    fn validates_refs() {
        assert!(validate_ref("master").is_ok());
        assert!(validate_ref("HEAD~2").is_ok());
        assert!(validate_ref("a1b2c3d").is_ok());
        assert!(validate_ref("HEAD@{1 week ago}").is_ok());
        assert!(validate_ref("").is_err());
        assert!(validate_ref("-x").is_err());
        assert!(validate_ref("a;rm -rf").is_err());
    }

    #[test]
    fn parses_diff_hunks() {
        let diff = "\
diff --git a/src/Foo.java b/src/Foo.java
--- a/src/Foo.java
+++ b/src/Foo.java
@@ -10,0 +11,3 @@
@@ -20,2 +24 @@
diff --git a/src/New.java b/src/New.java
new file mode 100644
--- /dev/null
+++ b/src/New.java
@@ -0,0 +1,5 @@
";
        let mut ranges = HashMap::new();
        let mut added = HashSet::new();
        parse_diff(diff, &mut ranges, &mut added);
        assert_eq!(ranges.get("src/Foo.java"), Some(&vec![(11, 13), (24, 24)]));
        assert_eq!(ranges.get("src/New.java"), Some(&vec![(1, 5)]));
        assert!(added.contains("src/New.java"));
    }
}
