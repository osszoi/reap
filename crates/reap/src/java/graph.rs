use crate::java::model::FileInfo;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct ModuleGraph {
    pub files: Vec<FileInfo>,
    pub edges: Vec<HashSet<usize>>,
    pub reverse: Vec<Vec<usize>>,
    pub reachable: Vec<bool>,
    pub is_root: Vec<bool>,
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
        let edges = build_edges(&files, &fqn_index);
        let reverse = build_reverse(&edges);
        let is_root = compute_roots(&files);
        let reachable = compute_reachable(&edges, &is_root);
        ModuleGraph { files, edges, reverse, reachable, is_root }
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

fn build_edges(files: &[FileInfo], fqn_index: &HashMap<String, usize>) -> Vec<HashSet<usize>> {
    files
        .iter()
        .enumerate()
        .map(|(id, file)| {
            let mut targets = HashSet::new();
            let add = |fqn: &str, targets: &mut HashSet<usize>| {
                if let Some(&tid) = fqn_index.get(fqn) {
                    if tid != id {
                        targets.insert(tid);
                    }
                }
            };

            for imp in &file.imports {
                add(imp, &mut targets);
            }
            for stat in &file.static_imports {
                add(stat, &mut targets);
                if let Some((parent, _)) = stat.rsplit_once('.') {
                    add(parent, &mut targets);
                }
            }
            for pkg in &file.wildcard_imports {
                for name in &file.referenced {
                    add(&format!("{pkg}.{name}"), &mut targets);
                }
            }
            if !file.package.is_empty() {
                for name in &file.referenced {
                    add(&format!("{}.{}", file.package, name), &mut targets);
                }
            }
            targets
        })
        .collect()
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
