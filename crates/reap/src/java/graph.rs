use crate::java::model::FileInfo;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct ModuleGraph {
    pub files: Vec<FileInfo>,
    pub edges: Vec<HashSet<usize>>,
    // edges minus the ones silenced by reap-ignore comments, only for cycle detection
    pub cycle_edges: Vec<HashSet<usize>>,
    pub cycle_ignores: Vec<IgnoreUse>,
    pub reverse: Vec<Vec<usize>>,
    pub reachable: Vec<bool>,
    pub is_root: Vec<bool>,
    fqn_index: HashMap<String, usize>,
}

pub struct IgnoreUse {
    pub file: usize,
    pub line: u32,
    pub whole_file: bool,
    pub targets: Vec<usize>,
}

#[derive(Default)]
struct FileEdges {
    all: HashSet<usize>,
    ignored: HashMap<u32, HashSet<usize>>,
}

const ROOT_ANNOTATIONS: &[&str] = &[
    "Component",
    "Service",
    "Repository",
    "Controller",
    "RestController",
    "Configuration",
    "Bean",
    "SpringBootApplication",
    "Entity",
    "Mapper",
    "Path",
];

impl ModuleGraph {
    pub fn build(files: Vec<FileInfo>) -> Self {
        let fqn_index = build_fqn_index(&files);
        let (edges, cycle_edges, cycle_ignores) = split_ignored(&files, build_edges(&files, &fqn_index));
        let reverse = build_reverse(&edges);
        let is_root = compute_roots(&files);
        let reachable = compute_reachable(&edges, &is_root);
        ModuleGraph { files, edges, cycle_edges, cycle_ignores, reverse, reachable, is_root, fqn_index }
    }

    // edges `file` would have if it sat at slot `id`, e.g. the base version of a changed file
    pub fn edges_for(&self, id: usize, file: &FileInfo) -> HashSet<usize> {
        file_edges(id, file, &self.fqn_index).all
    }

    pub fn fan_in(&self, id: usize) -> usize {
        self.reverse[id].len()
    }

    pub fn fan_out(&self, id: usize) -> usize {
        self.edges[id].len()
    }
}

fn build_fqn_index(files: &[FileInfo]) -> HashMap<String, usize> {
    let mut index = HashMap::new();
    for (id, file) in files.iter().enumerate() {
        for ty in &file.types {
            index.entry(ty.fqn.clone()).or_insert(id);
        }
    }
    index
}

fn build_edges(files: &[FileInfo], fqn_index: &HashMap<String, usize>) -> Vec<FileEdges> {
    files.iter().enumerate().map(|(id, file)| file_edges(id, file, fqn_index)).collect()
}

fn file_edges(id: usize, file: &FileInfo, fqn_index: &HashMap<String, usize>) -> FileEdges {
    let mut edges = FileEdges::default();
    let mut add = |fqn: &str, ignore_keys: &[&str]| {
        if let Some(&tid) = fqn_index.get(fqn) {
            if tid != id {
                edges.all.insert(tid);
                for line in file.cycle_ignore.comments_for(ignore_keys) {
                    edges.ignored.entry(line).or_default().insert(tid);
                }
            }
        }
    };

    for imp in &file.imports {
        add(imp, &[imp, last_segment(imp)]);
    }
    for stat in &file.static_imports {
        add(stat, &[stat, last_segment(stat)]);
        if let Some((parent, _)) = stat.rsplit_once('.') {
            add(parent, &[stat, last_segment(parent)]);
        }
    }
    for pkg in &file.wildcard_imports {
        for name in &file.referenced {
            add(&format!("{pkg}.{name}"), &[pkg, name]);
        }
    }
    if !file.package.is_empty() {
        for name in &file.referenced {
            add(&format!("{}.{}", file.package, name), &[name]);
        }
    }
    edges
}

fn last_segment(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

fn split_ignored(
    files: &[FileInfo],
    per_file: Vec<FileEdges>,
) -> (Vec<HashSet<usize>>, Vec<HashSet<usize>>, Vec<IgnoreUse>) {
    let mut edges = Vec::with_capacity(per_file.len());
    let mut cycle_edges = Vec::with_capacity(per_file.len());
    let mut uses = Vec::new();
    for (id, (fe, file)) in per_file.into_iter().zip(files).enumerate() {
        let ignore = &file.cycle_ignore;
        let mut cycle = fe.all.clone();
        if let Some(line) = ignore.file_comment {
            cycle.clear();
            uses.push(IgnoreUse { file: id, line, whole_file: true, targets: fe.all.iter().copied().collect() });
        }
        for &line in &ignore.next_line_comments {
            let targets: Vec<usize> = fe.ignored.get(&line).map(|t| t.iter().copied().collect()).unwrap_or_default();
            for t in &targets {
                cycle.remove(t);
            }
            uses.push(IgnoreUse { file: id, line, whole_file: false, targets });
        }
        edges.push(fe.all);
        cycle_edges.push(cycle);
    }
    (edges, cycle_edges, uses)
}

fn build_reverse(edges: &[HashSet<usize>]) -> Vec<Vec<usize>> {
    let mut reverse = vec![Vec::new(); edges.len()];
    for (src, targets) in edges.iter().enumerate() {
        for &tgt in targets {
            reverse[tgt].push(src);
        }
    }
    reverse
}

fn compute_roots(files: &[FileInfo]) -> Vec<bool> {
    let mut roots: Vec<bool> = files.iter().map(is_root_file).collect();
    if !roots.iter().any(|&r| r) {
        for (id, file) in files.iter().enumerate() {
            if file.types.iter().any(|t| t.is_public) {
                roots[id] = true;
            }
        }
    }
    roots
}

fn is_root_file(file: &FileInfo) -> bool {
    let path = file.path.to_string_lossy();
    if path.contains("/src/test/") || path.contains("/test/") {
        return true;
    }
    if file.functions.iter().any(|f| f.name == "main") {
        return true;
    }
    file.annotations.iter().any(|a| ROOT_ANNOTATIONS.contains(&a.as_str()))
}

fn compute_reachable(edges: &[HashSet<usize>], is_root: &[bool]) -> Vec<bool> {
    let mut reachable = vec![false; edges.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for (id, &root) in is_root.iter().enumerate() {
        if root {
            reachable[id] = true;
            queue.push_back(id);
        }
    }
    while let Some(id) = queue.pop_front() {
        for &tgt in &edges[id] {
            if !reachable[tgt] {
                reachable[tgt] = true;
                queue.push_back(tgt);
            }
        }
    }
    reachable
}
