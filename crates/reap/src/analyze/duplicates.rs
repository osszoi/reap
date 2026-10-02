use crate::baseline::{BaseFile, Baseline};
use crate::java::discover::java_files;
use crate::java::parse::parse;
use crate::types::{CloneFamily, CloneGroup, CloneInstance};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;
use tree_sitter::Node;

pub const DEFAULT_MIN_TOKENS: usize = 50;
pub const DEFAULT_MIN_LINES: u32 = 5;

struct FileTokens {
    path: String,
    keys: Vec<String>,
    lines: Vec<u32>,
    weights: Vec<f32>,
}

// decorators/annotations count for less than real code: similar @GetMapping etc.
// across files isn't duplication worth acting on
const ANNOTATION_WEIGHT: f32 = 0.5;

pub struct DuplicationReport {
    pub groups: Vec<CloneGroup>,
    pub families: Vec<CloneFamily>,
}

pub fn analyze(
    root: &Path,
    cwd: &Path,
    min_tokens: usize,
    min_lines: u32,
    baseline: Option<&Baseline>,
) -> DuplicationReport {
    let files: Vec<FileTokens> = java_files(root)
        .par_iter()
        .filter_map(|p| {
            let source = fs::read_to_string(p).ok()?;
            let rel = p.strip_prefix(cwd).unwrap_or(p).to_string_lossy().into_owned();
            tokens_of(rel, &source)
        })
        .filter(|f| !f.keys.is_empty())
        .collect();

    // merge-base versions of what the branch modified or deleted; unchanged files are the same on both sides
    let base_files: Vec<FileTokens> = baseline
        .map(|b| b.sources().collect::<Vec<_>>().par_iter().filter_map(|(p, s)| tokens_of(p.to_string(), s)).collect())
        .unwrap_or_default();
    let corpus = baseline.map(|b| {
        let unchanged = files.iter().filter(|f| matches!(b.file(&f.path), BaseFile::Unchanged));
        BaseCorpus::new(unchanged.chain(&base_files).map(|f| f.keys.as_slice()))
    });

    let groups = detect(&files, min_tokens, min_lines, corpus.as_ref());
    // PR mode: a family is built only from the copies the branch introduced
    let fresh: Vec<CloneGroup> = groups.iter().filter(|g| !g.existed_at_base).cloned().collect();
    let families = group_families(&fresh);
    DuplicationReport { groups, families }
}

fn tokens_of(path: String, source: &str) -> Option<FileTokens> {
    let tree = parse(source)?;
    let mut keys = Vec::new();
    let mut lines = Vec::new();
    let mut weights = Vec::new();
    tokenize(tree.root_node(), source.as_bytes(), &mut keys, &mut lines, &mut weights, 1.0);
    Some(FileTokens { path, keys, lines, weights })
}

// token stream of the merge-base, to tell a copy that was already there from one the branch added
struct BaseCorpus<'a> {
    ids: HashMap<&'a str, u32>,
    // 0 separates files
    text: Vec<u32>,
    at: HashMap<u32, Vec<usize>>,
}

impl<'a> BaseCorpus<'a> {
    fn new(files: impl Iterator<Item = &'a [String]>) -> Self {
        let mut ids: HashMap<&'a str, u32> = HashMap::new();
        let mut text = Vec::new();
        for keys in files {
            for key in keys {
                let next = ids.len() as u32 + 1;
                text.push(*ids.entry(key.as_str()).or_insert(next));
            }
            text.push(0);
        }
        let mut at: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, &t) in text.iter().enumerate() {
            if t != 0 {
                at.entry(t).or_default().push(i);
            }
        }
        BaseCorpus { ids, text, at }
    }

    // non-overlapping occurrences, same rule build_group uses for instances
    fn count(&self, pattern: &[String]) -> usize {
        let Some(ids) = pattern.iter().map(|k| self.ids.get(k.as_str()).copied()).collect::<Option<Vec<u32>>>() else {
            return 0;
        };
        // anchor on the rarest token so common ones like `{` don't blow up the scan
        let Some((anchor, positions)) =
            ids.iter().enumerate().filter_map(|(i, id)| self.at.get(id).map(|v| (i, v))).min_by_key(|(_, v)| v.len())
        else {
            return 0;
        };
        let mut count = 0;
        let mut next_free = 0;
        for &p in positions {
            if p < anchor || p - anchor < next_free {
                continue;
            }
            let start = p - anchor;
            if self.text.get(start..start + ids.len()) == Some(ids.as_slice()) {
                count += 1;
                next_free = start + ids.len();
            }
        }
        count
    }
}

// --- tokenization ---

fn tokenize(
    node: Node,
    src: &[u8],
    keys: &mut Vec<String>,
    lines: &mut Vec<u32>,
    weights: &mut Vec<f32>,
    weight: f32,
) {
    let kind = node.kind();
    if kind == "line_comment" || kind == "block_comment" {
        return;
    }
    // imports and package decls are near-identical across files in a module — never duplication
    if kind == "import_declaration" || kind == "package_declaration" {
        return;
    }
    // discount everything under an annotation so decorators contribute half
    let weight = if kind == "annotation" || kind == "marker_annotation" {
        ANNOTATION_WEIGHT
    } else {
        weight
    };
    let line = node.start_position().row as u32 + 1;
    if kind.ends_with("literal") {
        let text = node.utf8_text(src).unwrap_or("");
        keys.push(format!("{kind}:{text}"));
        lines.push(line);
        weights.push(weight);
        return;
    }
    if node.child_count() == 0 {
        if kind == "identifier" || kind == "type_identifier" {
            keys.push(format!("id:{}", node.utf8_text(src).unwrap_or("")));
        } else {
            keys.push(kind.to_string());
        }
        lines.push(line);
        weights.push(weight);
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        tokenize(child, src, keys, lines, weights, weight);
    }
}

// --- detection via generalized suffix array + LCP ---

fn detect(files: &[FileTokens], min_tokens: usize, min_lines: u32, base: Option<&BaseCorpus>) -> Vec<CloneGroup> {
    if files.is_empty() {
        return Vec::new();
    }
    let mut interner: HashMap<&str, i64> = HashMap::new();
    let mut next_id: i64 = 1;
    let mut text: Vec<i64> = Vec::new();
    let mut owner: Vec<usize> = Vec::new();
    let mut offset: Vec<usize> = Vec::new();
    let mut sentinel: i64 = -1;

    for (fid, f) in files.iter().enumerate() {
        for (i, key) in f.keys.iter().enumerate() {
            let id = *interner.entry(key.as_str()).or_insert_with(|| {
                let v = next_id;
                next_id += 1;
                v
            });
            text.push(id);
            owner.push(fid);
            offset.push(i);
        }
        text.push(sentinel);
        owner.push(usize::MAX);
        offset.push(0);
        sentinel -= 1;
    }

    let sa = suffix_array(&text);
    let lcp = kasai(&text, &sa);

    let mut groups = Vec::new();
    let n = text.len();
    let mut i = 1;
    while i < n {
        if lcp[i] >= min_tokens {
            let start = i - 1;
            let mut j = i;
            let mut min_len = lcp[i];
            while j < n && lcp[j] >= min_tokens {
                min_len = min_len.min(lcp[j]);
                j += 1;
            }
            let positions: Vec<usize> = (start..j).map(|x| sa[x]).collect();
            if let Some(group) =
                build_group(&positions, min_len, files, &owner, &offset, min_tokens, min_lines, base)
            {
                groups.push(group);
            }
            i = j;
        } else {
            i += 1;
        }
    }

    remove_line_subsets(&mut groups);
    groups.sort_by(|a, b| b.line_count.cmp(&a.line_count).then(b.token_count.cmp(&a.token_count)));
    groups
}

fn build_group(
    positions: &[usize],
    length: usize,
    files: &[FileTokens],
    owner: &[usize],
    offset: &[usize],
    min_tokens: usize,
    min_lines: u32,
    base: Option<&BaseCorpus>,
) -> Option<CloneGroup> {
    let mut spans: Vec<(usize, usize)> = positions
        .iter()
        .filter(|&&p| owner[p] != usize::MAX)
        .map(|&p| (owner[p], offset[p]))
        .collect();
    spans.sort_unstable();

    // weighted gate: a block that's mostly decorators/imports-residue shouldn't count.
    // tokens are identical across instances, so any one span is representative.
    let &(first_fid, first_off) = spans.first()?;
    let first = &files[first_fid];
    let first_end = (first_off + length).min(first.weights.len());
    let weighted: f32 = first.weights[first_off..first_end].iter().sum();
    if weighted < min_tokens as f32 {
        return None;
    }

    let mut instances: Vec<CloneInstance> = Vec::new();
    let mut last: Option<(usize, usize)> = None;
    for (fid, off) in spans {
        if let Some((lf, lo)) = last {
            if lf == fid && off < lo + length {
                continue;
            }
        }
        let f = &files[fid];
        let end_tok = (off + length - 1).min(f.lines.len().saturating_sub(1));
        let start_line = f.lines[off];
        let end_line = f.lines[end_tok];
        instances.push(CloneInstance { file: f.path.clone(), start_line, end_line });
        last = Some((fid, off));
    }

    if instances.len() < 2 {
        return None;
    }
    let line_count = instances.iter().map(|i| i.end_line - i.start_line + 1).max().unwrap_or(0);
    if line_count < min_lines {
        return None;
    }
    // compare without the edges: a move or a nearby edit can glue a `}` or two onto old copies,
    // and that alone shouldn't make them new
    let pattern = &first.keys[first_off..first_end];
    let trim = pattern.len() / 10;
    let existed_at_base = base.is_some_and(|b| b.count(&pattern[trim..pattern.len() - trim]) >= instances.len());
    Some(CloneGroup { instances, token_count: length, line_count, existed_at_base })
}

fn remove_line_subsets(groups: &mut Vec<CloneGroup>) {
    groups.sort_by(|a, b| {
        b.line_count.cmp(&a.line_count).then(b.token_count.cmp(&a.token_count))
    });
    let mut kept: Vec<(String, u32, u32)> = Vec::new();
    groups.retain(|g| {
        let contained = g.instances.iter().all(|i| {
            kept.iter().any(|(f, ks, ke)| {
                *f == i.file && *ks <= i.start_line && i.end_line <= *ke
            })
        });
        if !contained {
            for i in &g.instances {
                kept.push((i.file.clone(), i.start_line, i.end_line));
            }
        }
        !contained
    });
}

// --- clone families: bucket by identical file-set ---

fn group_families(groups: &[CloneGroup]) -> Vec<CloneFamily> {
    let mut buckets: Vec<(BTreeSet<String>, Vec<&CloneGroup>)> = Vec::new();
    for g in groups {
        let set: BTreeSet<String> = g.instances.iter().map(|i| i.file.clone()).collect();
        match buckets.iter_mut().find(|(s, _)| *s == set) {
            Some((_, v)) => v.push(g),
            None => buckets.push((set, vec![g])),
        }
    }

    let mut families: Vec<CloneFamily> = buckets
        .into_iter()
        .filter(|(_, gs)| gs.len() > 1)
        .map(|(set, gs)| {
            let total_lines: u32 = gs.iter().map(|g| g.line_count).sum();
            let suggestion = if total_lines >= 50 {
                "Extract a shared module".to_string()
            } else {
                "Extract a shared method".to_string()
            };
            CloneFamily {
                files: set.into_iter().collect(),
                group_count: gs.len(),
                total_lines,
                suggestion,
                members: gs.iter().map(|g| (*g).clone()).collect(),
            }
        })
        .collect();

    families.sort_by(|a, b| b.total_lines.cmp(&a.total_lines).then(b.group_count.cmp(&a.group_count)));
    families
}

// --- suffix array (prefix doubling) + Kasai LCP ---

fn suffix_array(s: &[i64]) -> Vec<usize> {
    let n = s.len();
    if n == 0 {
        return Vec::new();
    }
    let mut sa: Vec<usize> = (0..n).collect();
    let mut rank: Vec<i64> = s.to_vec();
    let mut k = 1usize;
    loop {
        let key: Vec<(i64, i64)> =
            (0..n).map(|i| (rank[i], if i + k < n { rank[i + k] } else { -1 })).collect();
        sa.sort_by(|&a, &b| key[a].cmp(&key[b]));
        let mut tmp = vec![0i64; n];
        for i in 1..n {
            tmp[sa[i]] = tmp[sa[i - 1]] + if key[sa[i]] > key[sa[i - 1]] { 1 } else { 0 };
        }
        rank = tmp;
        if rank[sa[n - 1]] == (n - 1) as i64 {
            break;
        }
        k <<= 1;
        if k >= n {
            break;
        }
    }
    sa
}

fn kasai(s: &[i64], sa: &[usize]) -> Vec<usize> {
    let n = s.len();
    let mut rank = vec![0usize; n];
    for (i, &p) in sa.iter().enumerate() {
        rank[p] = i;
    }
    let mut lcp = vec![0usize; n];
    let mut h = 0usize;
    for i in 0..n {
        if rank[i] > 0 {
            let j = sa[rank[i] - 1];
            while i + h < n && j + h < n && s[i + h] == s[j + h] {
                h += 1;
            }
            lcp[rank[i]] = h;
            if h > 0 {
                h -= 1;
            }
        } else {
            h = 0;
        }
    }
    lcp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn toks(source: &str) -> FileTokens {
        let tree = parse(source).unwrap();
        let mut keys = Vec::new();
        let mut lines = Vec::new();
        let mut weights = Vec::new();
        tokenize(tree.root_node(), source.as_bytes(), &mut keys, &mut lines, &mut weights, 1.0);
        FileTokens { path: "X.java".into(), keys, lines, weights }
    }

    #[test]
    fn detects_identical_block_across_files() {
        let block = r#"
class %NAME% {
  int run(int a, int b) {
    int total = 0;
    for (int i = 0; i < a; i++) {
      if (i % 2 == 0) { total += i * b; } else { total -= i; }
    }
    while (total > 100) { total = total - b; }
    return total;
  }
}
"#;
        let a = toks(&block.replace("%NAME%", "A"));
        let b = toks(&block.replace("%NAME%", "B"));
        // ~50+ tokens of shared body; use a modest threshold
        let files = vec![
            FileTokens { path: "A.java".into(), keys: a.keys, lines: a.lines, weights: a.weights },
            FileTokens { path: "B.java".into(), keys: b.keys, lines: b.lines, weights: b.weights },
        ];
        let groups = detect(&files, 20, 3, None);
        assert!(!groups.is_empty(), "expected a clone group");
        let g = &groups[0];
        assert!(g.instances.len() >= 2, "expected >=2 instances");
        let _ = PathBuf::new();
    }

    const BLOCK: &str = r#"
  int %NAME%(int a, int b) {
    int total = 0;
    for (int i = 0; i < a; i++) {
      if (i % 2 == 0) { total += i * b; } else { total -= i; }
    }
    while (total > 100) { total = total - b; }
    return total;
  }
"#;

    fn file(path: &str, methods: &[&str]) -> FileTokens {
        let body: String = methods.iter().map(|m| BLOCK.replace("%NAME%", m)).collect();
        let class = path.trim_end_matches(".java");
        let mut t = toks(&format!("class {class} {{\n{body}}}\n"));
        t.path = path.into();
        t
    }

    fn groups_vs_base(head: &[FileTokens], base: &[FileTokens]) -> Vec<CloneGroup> {
        let corpus = BaseCorpus::new(base.iter().map(|f| f.keys.as_slice()));
        detect(head, 20, 3, Some(&corpus))
    }

    #[test]
    fn old_copies_existed_at_base() {
        let base = vec![file("A.java", &["run", "run"])];
        let head = vec![file("A.java", &["run", "run"])];
        let groups = groups_vs_base(&head, &base);
        assert!(!groups.is_empty());
        assert!(groups.iter().all(|g| g.existed_at_base));
    }

    #[test]
    fn extra_copy_is_introduced() {
        let base = vec![file("A.java", &["run", "run"])];
        let head = vec![file("A.java", &["run", "run"]), file("B.java", &["run"])];
        let groups = groups_vs_base(&head, &base);
        assert!(groups.iter().any(|g| g.instances.len() == 3 && !g.existed_at_base));
    }

    #[test]
    fn brand_new_duplicate_is_introduced() {
        let base = vec![file("A.java", &["run"])];
        let head = vec![file("A.java", &["run"]), file("B.java", &["run"])];
        assert!(groups_vs_base(&head, &base).iter().all(|g| !g.existed_at_base));
    }

    #[test]
    fn copy_moved_to_another_file_still_existed() {
        let base = vec![file("A.java", &["run", "run"])];
        let head = vec![file("A.java", &["run"]), file("Moved.java", &["run"])];
        assert!(groups_vs_base(&head, &base).iter().all(|g| g.existed_at_base));
    }

    #[test]
    fn corpus_counts_non_overlapping_occurrences() {
        let keys = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let files = [keys("a b a b a"), keys("x a b a")];
        let corpus = BaseCorpus::new(files.iter().map(|k| k.as_slice()));
        assert_eq!(corpus.count(&keys("a b a")), 2);
        assert_eq!(corpus.count(&keys("b a")), 3);
        assert_eq!(corpus.count(&keys("a z")), 0);
    }
}
