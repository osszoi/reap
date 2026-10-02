use std::collections::HashMap;
use tree_sitter::Node;

const NEXT_LINE: &str = "reap-ignore-next-line";
const WHOLE_FILE: &str = "reap-ignore-file";
const CIRCULAR: &[&str] = &["circular", "circular-dependency", "circular-dependencies", "cycles"];

#[derive(Debug, Default)]
pub struct CycleIgnore {
    pub file_comment: Option<u32>,
    pub next_line_comments: Vec<u32>,
    pub unknown_rules: Vec<(u32, String)>,
    targets: HashMap<u32, u32>,
    // import spec or simple name written on an ignored line -> comment lines that ignore it
    keys: HashMap<String, Vec<u32>>,
}

impl CycleIgnore {
    pub fn comments_for(&self, keys: &[&str]) -> Vec<u32> {
        if self.keys.is_empty() {
            return Vec::new();
        }
        keys.iter().filter_map(|k| self.keys.get(*k)).flatten().copied().collect()
    }
}

pub fn collect(root: Node, src: &[u8]) -> CycleIgnore {
    let mut ignore = CycleIgnore::default();
    walk(root, &mut |n| {
        if n.kind().ends_with("comment") {
            parse_comment(n, src, &mut ignore);
        }
    });
    if !ignore.targets.is_empty() {
        walk(root, &mut |n| mark_key(n, src, &mut ignore));
    }
    ignore
}

fn walk<'t>(node: Node<'t>, f: &mut impl FnMut(Node<'t>)) {
    f(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, f);
    }
}

fn parse_comment(node: Node, src: &[u8], ignore: &mut CycleIgnore) {
    let Ok(text) = node.utf8_text(src) else { return };
    let body = text
        .trim_start_matches('/')
        .trim_start_matches('*')
        .trim_end_matches('/')
        .trim_end_matches('*')
        .trim();
    let (rest, whole_file) = match (strip_marker(body, WHOLE_FILE), strip_marker(body, NEXT_LINE)) {
        (Some(rest), _) => (rest, true),
        (None, Some(rest)) => (rest, false),
        _ => return,
    };

    let comment_line = node.start_position().row as u32 + 1;
    // anything after `--` is a free-text reason, eslint style
    let rules: Vec<&str> = rest
        .split("--")
        .next()
        .unwrap_or("")
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .collect();
    let mut circular = rules.is_empty();
    for rule in rules {
        if CIRCULAR.contains(&rule) {
            circular = true;
        } else {
            ignore.unknown_rules.push((comment_line, rule.to_string()));
        }
    }
    if !circular {
        return;
    }

    if whole_file {
        ignore.file_comment.get_or_insert(comment_line);
    } else {
        ignore.targets.insert(node.end_position().row as u32 + 2, comment_line);
        ignore.next_line_comments.push(comment_line);
    }
}

fn strip_marker<'a>(body: &'a str, marker: &str) -> Option<&'a str> {
    let rest = body.strip_prefix(marker)?;
    (rest.is_empty() || rest.starts_with(char::is_whitespace)).then_some(rest)
}

fn mark_key(node: Node, src: &[u8], ignore: &mut CycleIgnore) {
    let key = match node.kind() {
        "import_declaration" => node.named_child(0),
        "identifier" | "type_identifier" => Some(node),
        _ => None,
    };
    let Some(key) = key else { return };
    let Some(&comment) = ignore.targets.get(&(node.start_position().row as u32 + 1)) else { return };
    if let Ok(text) = key.utf8_text(src) {
        let lines = ignore.keys.entry(text.to_string()).or_default();
        if !lines.contains(&comment) {
            lines.push(comment);
        }
    }
}
