use crate::baseline::{BaseFile, Baseline};
use crate::java::graph::ModuleGraph;
use crate::types::{CircularDependency, IgnoreNote};
use std::collections::HashSet;
use std::path::Path;

pub struct CircularReport {
    pub cycles: Vec<CircularDependency>,
    // (file, comment line) per dependency edge a reap-ignore actually took out of a cycle
    pub ignored_edges: Vec<(String, u32)>,
    pub notes: Vec<IgnoreNote>,
}

pub fn collect(graph: &ModuleGraph, cwd: &Path, baseline: Option<&Baseline>) -> CircularReport {
    let sccs = tarjan_scc(&graph.cycle_edges);
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
                new_edges: baseline.map(|b| introduced_edges(graph, &scc, b, cwd)).unwrap_or_default(),
            }
        })
        .collect();

    cycles.sort_by(|a, b| a.files.len().cmp(&b.files.len()).then(a.files.cmp(&b.files)));
    let (ignored_edges, mut notes) = ignore_usage(graph, cwd);
    notes.extend(unknown_rules(graph, cwd));
    notes.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    CircularReport { cycles, ignored_edges, notes }
}

fn introduced_edges(graph: &ModuleGraph, scc: &[usize], baseline: &Baseline, cwd: &Path) -> Vec<(String, String)> {
    let members: HashSet<usize> = scc.iter().copied().collect();
    let is_new = |id: usize| matches!(baseline.file(&rel(graph, id, cwd)), BaseFile::New);
    let mut out = Vec::new();
    for &from in scc {
        let path = rel(graph, from, cwd);
        let before = match baseline.file(&path) {
            BaseFile::Unchanged => None,
            BaseFile::New => Some(HashSet::new()),
            BaseFile::Changed(base) => Some(graph.edges_for(from, base)),
        };
        for &to in graph.cycle_edges[from].iter().filter(|t| members.contains(t)) {
            let existed = before.as_ref().is_none_or(|b| b.contains(&to));
            if !existed || is_new(to) {
                out.push((path.clone(), rel(graph, to, cwd)));
            }
        }
    }
    out.sort();
    out
}

// an ignore is stale when none of the edges it silenced sits inside a cycle of the unfiltered graph
fn ignore_usage(graph: &ModuleGraph, cwd: &Path) -> (Vec<(String, u32)>, Vec<IgnoreNote>) {
    let mut ignored = Vec::new();
    let mut notes = Vec::new();
    if graph.cycle_ignores.is_empty() {
        return (ignored, notes);
    }
    let mut component = vec![0usize; graph.files.len()];
    for (c, scc) in tarjan_scc(&graph.edges).into_iter().enumerate() {
        for id in scc {
            component[id] = c;
        }
    }

    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for usage in &graph.cycle_ignores {
        let file = rel(graph, usage.file, cwd);
        let in_cycle: Vec<usize> = usage
            .targets
            .iter()
            .copied()
            .filter(|&t| component[t] == component[usage.file])
            .collect();
        if in_cycle.is_empty() {
            let message = if usage.whole_file {
                "stale reap-ignore-file: this file isn't part of any cycle"
            } else if usage.targets.is_empty() {
                "reap-ignore-next-line matched nothing: the next line names no project class"
            } else {
                "stale reap-ignore-next-line: no cycle runs through this dependency"
            };
            notes.push(IgnoreNote { file, line: usage.line, message: message.into() });
            continue;
        }
        for t in in_cycle {
            if seen.insert((usage.file, t)) {
                ignored.push((file.clone(), usage.line));
            }
        }
    }
    (ignored, notes)
}

fn unknown_rules<'a>(graph: &'a ModuleGraph, cwd: &'a Path) -> impl Iterator<Item = IgnoreNote> + 'a {
    graph.files.iter().enumerate().flat_map(move |(id, f)| {
        f.cycle_ignore.unknown_rules.iter().map(move |(line, rule)| IgnoreNote {
            file: rel(graph, id, cwd),
            line: *line,
            message: format!("unknown reap-ignore rule \"{rule}\" (supported: circular)"),
        })
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::baseline::Baseline;
    use crate::java::model::parse_file;
    use std::path::PathBuf;

    fn report(sources: &[(&str, &str)]) -> CircularReport {
        let files = sources
            .iter()
            .map(|(path, src)| parse_file(PathBuf::from(format!("/r/{path}")), src).unwrap())
            .collect();
        collect(&ModuleGraph::build(files), Path::new("/r"), None)
    }

    const B_IMPORTS_A: (&str, &str) = ("b/B.java", "package b;\nimport a.A;\nclass B { A a; }\n");

    #[test]
    fn next_line_on_import_breaks_cycle() {
        let r = report(&[
            ("a/A.java", "package a;\n// reap-ignore-next-line circular\nimport b.B;\nclass A { B b; }\n"),
            B_IMPORTS_A,
        ]);
        assert!(r.cycles.is_empty());
        assert_eq!(r.ignored_edges, vec![("a/A.java".to_string(), 2)]);
        assert!(r.notes.is_empty());
    }

    #[test]
    fn next_line_on_same_package_field_breaks_cycle() {
        let r = report(&[
            ("p/A.java", "package p;\nclass A {\n  // reap-ignore-next-line circular -- JPA back-reference\n  private B b;\n  B other() { return b; }\n}\n"),
            ("p/B.java", "package p;\nclass B { private A a; }\n"),
        ]);
        assert!(r.cycles.is_empty());
        assert!(r.notes.is_empty());
    }

    #[test]
    fn comment_above_annotation_matches_nothing() {
        let r = report(&[
            ("p/A.java", "package p;\nclass A {\n  // reap-ignore-next-line circular\n  @Autowired\n  private B b;\n}\n"),
            ("p/B.java", "package p;\nclass B { private A a; }\n"),
        ]);
        assert_eq!(r.cycles.len(), 1);
        assert_eq!(r.notes.len(), 1);
        assert!(r.notes[0].message.contains("matched nothing"));
        assert_eq!(r.notes[0].line, 3);
    }

    #[test]
    fn other_path_keeps_cycle() {
        let r = report(&[
            ("a/A.java", "package a;\n// reap-ignore-next-line circular\nimport b.B;\nimport c.C;\nclass A { B b; C c; }\n"),
            B_IMPORTS_A,
            ("c/C.java", "package c;\nimport b.B;\nclass C { B b; }\n"),
        ]);
        assert_eq!(r.cycles.len(), 1);
        assert_eq!(r.cycles[0].files, vec!["a/A.java", "b/B.java", "c/C.java"]);
        assert_eq!(r.ignored_edges.len(), 1);
    }

    #[test]
    fn whole_file_drops_only_that_file() {
        let r = report(&[
            ("p/A.java", "package p;\nclass A { B b; }\n"),
            ("p/B.java", "package p;\nclass B { A a; C c; }\n"),
            ("p/C.java", "/* reap-ignore-file circular */\npackage p;\nclass C { B b; }\n"),
        ]);
        assert_eq!(r.cycles.len(), 1);
        assert_eq!(r.cycles[0].files, vec!["p/A.java", "p/B.java"]);
        assert_eq!(r.ignored_edges, vec![("p/C.java".to_string(), 1)]);
    }

    #[test]
    fn stale_ignores_are_reported() {
        let r = report(&[
            ("a/A.java", "// reap-ignore-file circular\npackage a;\n// reap-ignore-next-line circular\nimport b.B;\nclass A { B b; }\n"),
            ("b/B.java", "package b;\nclass B {}\n"),
        ]);
        let messages: Vec<&str> = r.notes.iter().map(|n| n.message.as_str()).collect();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].starts_with("stale reap-ignore-file"));
        assert!(messages[1].starts_with("stale reap-ignore-next-line"));
        assert!(r.ignored_edges.is_empty());
    }

    #[test]
    fn unknown_rule_is_reported_and_ignored() {
        let r = report(&[
            ("a/A.java", "package a;\n// reap-ignore-next-line circlar\nimport b.B;\nclass A { B b; }\n"),
            B_IMPORTS_A,
        ]);
        assert_eq!(r.cycles.len(), 1);
        assert_eq!(r.notes.len(), 1);
        assert!(r.notes[0].message.contains("\"circlar\""));
    }

    #[test]
    fn bare_marker_means_all_rules() {
        let r = report(&[("a/A.java", "package a;\n// reap-ignore-next-line\nimport b.B;\nclass A { B b; }\n"), B_IMPORTS_A]);
        assert!(r.cycles.is_empty());
    }

    #[test]
    fn marker_must_start_the_comment() {
        let r = report(&[
            ("a/A.java", "package a;\n// TODO reap-ignore-next-line circular\nimport b.B;\n// reap-ignore-filex\nclass A { B b; }\n"),
            B_IMPORTS_A,
        ]);
        assert_eq!(r.cycles.len(), 1);
        assert!(r.notes.is_empty());
    }

    fn pr_report(head: &[(&str, &str)], base: &[(&str, Option<&str>)]) -> CircularReport {
        let files = head
            .iter()
            .map(|(path, src)| parse_file(PathBuf::from(format!("/r/{path}")), src).unwrap())
            .collect();
        let baseline = Baseline::from_sources(base, head);
        collect(&ModuleGraph::build(files), Path::new("/r"), Some(&baseline))
    }

    const A: &str = "package p;\nclass A { B b; }\n";
    const B: &str = "package p;\nclass B { C c; }\n";
    const C: &str = "package p;\nclass C { A a; }\n";

    #[test]
    fn touching_a_cycle_member_without_new_dependency_is_not_introduced() {
        let a_edited = "package p;\nclass A { B b; int x = 1; }\n";
        let r = pr_report(&[("p/A.java", a_edited), ("p/B.java", B), ("p/C.java", C)], &[("p/A.java", Some(A))]);
        assert_eq!(r.cycles.len(), 1);
        assert!(r.cycles[0].new_edges.is_empty());
    }

    #[test]
    fn new_dependency_inside_cycle_is_introduced() {
        let b_edited = "package p;\nclass B { C c; A back; }\n";
        let r = pr_report(&[("p/A.java", A), ("p/B.java", b_edited), ("p/C.java", C)], &[("p/B.java", Some(B))]);
        assert_eq!(r.cycles[0].new_edges, vec![("p/B.java".to_string(), "p/A.java".to_string())]);
    }

    #[test]
    fn new_file_joining_cycle_is_introduced() {
        let c_edited = "package p;\nclass C { A a; D d; }\n";
        let d = "package p;\nclass D { A a; }\n";
        let r = pr_report(
            &[("p/A.java", A), ("p/B.java", B), ("p/C.java", c_edited), ("p/D.java", d)],
            &[("p/C.java", Some(C)), ("p/D.java", None)],
        );
        assert_eq!(
            r.cycles[0].new_edges,
            vec![("p/C.java".to_string(), "p/D.java".to_string()), ("p/D.java".to_string(), "p/A.java".to_string())]
        );
    }

    #[test]
    fn ignore_does_not_touch_reachability_edges() {
        let files = vec![
            parse_file(PathBuf::from("/r/a/A.java"), "// reap-ignore-file circular\npackage a;\nimport b.B;\nclass A { B b; }\n").unwrap(),
            parse_file(PathBuf::from("/r/b/B.java"), B_IMPORTS_A.1).unwrap(),
        ];
        let graph = ModuleGraph::build(files);
        assert_eq!(graph.fan_out(0), 1);
        assert!(graph.cycle_edges[0].is_empty());
    }
}
