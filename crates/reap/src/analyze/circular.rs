use crate::java::graph::ModuleGraph;
use crate::types::CircularDependency;
use std::path::Path;

pub fn collect(graph: &ModuleGraph, cwd: &Path) -> Vec<CircularDependency> {
    let sccs = tarjan_scc(&graph.edges);
    let mut cycles: Vec<CircularDependency> = sccs
        .into_iter()
        .filter(|scc| scc.len() >= 2)
        .map(|mut scc| {
            scc.sort_by_key(|&id| rel(graph, id, cwd));
            let cross_package = scc
                .iter()
                .filter_map(|&id| graph.files[id].package.clone().into())
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1;
            CircularDependency {
                files: scc.iter().map(|&id| rel(graph, id, cwd)).collect(),
                cross_package,
            }
        })
        .collect();

    cycles.sort_by(|a, b| a.files.len().cmp(&b.files.len()).then(a.files.cmp(&b.files)));
    cycles
}

fn rel(graph: &ModuleGraph, id: usize, cwd: &Path) -> String {
    let p = &graph.files[id].path;
    p.strip_prefix(cwd).unwrap_or(p).to_string_lossy().into_owned()
}

// Tarjan's strongly-connected-components, iterative.
fn tarjan_scc(edges: &[std::collections::HashSet<usize>]) -> Vec<Vec<usize>> {
    let n = edges.len();
    let mut index = vec![usize::MAX; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    let mut counter = 0usize;

    let adj: Vec<Vec<usize>> = edges
        .iter()
        .map(|s| {
            let mut v: Vec<usize> = s.iter().copied().collect();
            v.sort_unstable();
            v
        })
        .collect();

    for start in 0..n {
        if index[start] != usize::MAX {
            continue;
        }
        let mut call_stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(&(v, pos)) = call_stack.last() {
            if pos == 0 {
                index[v] = counter;
                lowlink[v] = counter;
                counter += 1;
                stack.push(v);
                on_stack[v] = true;
            }
            if pos < adj[v].len() {
                let w = adj[v][pos];
                call_stack.last_mut().unwrap().1 += 1;
                if index[w] == usize::MAX {
                    call_stack.push((w, 0));
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(index[w]);
                }
            } else {
                if lowlink[v] == index[v] {
                    let mut scc = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        scc.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(scc);
                }
                call_stack.pop();
                if let Some(&(parent, _)) = call_stack.last() {
                    lowlink[parent] = lowlink[parent].min(lowlink[v]);
                }
            }
        }
    }
    sccs
}
