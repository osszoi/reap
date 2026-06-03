use crate::java::extract::{functions_in, FunctionMetrics};
use crate::java::parse::parse;
use std::collections::HashSet;
use std::path::PathBuf;
use tree_sitter::Node;

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub fqn: String,
    pub simple: String,
    pub line: u32,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct MemberDecl {
    pub name: String,
    pub line: u32,
}

#[derive(Debug)]
pub struct FileInfo {
    pub path: PathBuf,
    pub package: String,
    pub imports: Vec<String>,
    pub wildcard_imports: Vec<String>,
    pub static_imports: Vec<String>,
    pub types: Vec<TypeDecl>,
    pub referenced: HashSet<String>,
    pub used_names: HashSet<String>,
    pub exports: Vec<MemberDecl>,
    pub functions: Vec<FunctionMetrics>,
    pub line_count: u32,
    pub annotations: HashSet<String>,
}

const TYPE_DECL_KINDS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "record_declaration",
    "annotation_type_declaration",
];

pub fn parse_file(path: PathBuf, source: &str) -> Option<FileInfo> {
    let tree = parse(source)?;
    let root = tree.root_node();
    let src = source.as_bytes();

    let package = find_package(root, src);
    let (imports, wildcard_imports, static_imports) = collect_imports(root, src);
    let mut types = Vec::new();
    collect_types(root, src, &package, &mut Vec::new(), &mut types);
    let mut referenced = HashSet::new();
    let mut used_names = HashSet::new();
    let mut annotations = HashSet::new();
    collect_references(root, src, &mut referenced, &mut used_names, &mut annotations);
    let mut exports = Vec::new();
    collect_public_methods(root, src, &mut exports);

    Some(FileInfo {
        path,
        package,
        imports,
        wildcard_imports,
        static_imports,
        types,
        referenced,
        used_names,
        exports,
        functions: functions_in(root, src),
        line_count: source.split('\n').count() as u32,
        annotations,
    })
}

fn find_package(root: Node, src: &[u8]) -> String {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            let mut inner = child.walk();
            for c in child.named_children(&mut inner) {
                if let Ok(t) = c.utf8_text(src) {
                    return t.to_string();
                }
            }
        }
    }
    String::new()
}

fn collect_imports(root: Node, src: &[u8]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut imports = Vec::new();
    let mut wildcard = Vec::new();
    let mut statics = Vec::new();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "import_declaration" {
            continue;
        }
        let Ok(text) = child.utf8_text(src) else { continue };
        let spec = text
            .trim()
            .trim_start_matches("import")
            .trim()
            .trim_end_matches(';')
            .trim();
        let is_static = spec.starts_with("static");
        let spec = spec.trim_start_matches("static").trim();
        if let Some(pkg) = spec.strip_suffix(".*") {
            wildcard.push(pkg.to_string());
        } else if is_static {
            statics.push(spec.to_string());
        } else {
            imports.push(spec.to_string());
        }
    }
    (imports, wildcard, statics)
}

fn collect_types(
    node: Node,
    src: &[u8],
    package: &str,
    enclosing: &mut Vec<String>,
    out: &mut Vec<TypeDecl>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if TYPE_DECL_KINDS.contains(&child.kind()) {
            let simple = child
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .unwrap_or("")
                .to_string();
            if simple.is_empty() {
                collect_types(child, src, package, enclosing, out);
                continue;
            }
            let mut parts: Vec<&str> = Vec::new();
            if !package.is_empty() {
                parts.push(package);
            }
            for e in enclosing.iter() {
                parts.push(e);
            }
            parts.push(&simple);
            out.push(TypeDecl {
                fqn: parts.join("."),
                simple: simple.clone(),
                line: child.start_position().row as u32 + 1,
                is_public: has_modifier(child, src, "public"),
            });
            enclosing.push(simple);
            collect_types(child, src, package, enclosing, out);
            enclosing.pop();
        } else {
            collect_types(child, src, package, enclosing, out);
        }
    }
}

fn has_modifier(node: Node, src: &[u8], modifier: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut inner = child.walk();
            for m in child.children(&mut inner) {
                if m.utf8_text(src).map(|t| t == modifier).unwrap_or(false) {
                    return true;
                }
            }
        }
    }
    false
}

fn collect_references(
    node: Node,
    src: &[u8],
    referenced: &mut HashSet<String>,
    used_names: &mut HashSet<String>,
    annotations: &mut HashSet<String>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" => {
                if let Ok(t) = child.utf8_text(src) {
                    referenced.insert(t.to_string());
                    used_names.insert(t.to_string());
                }
            }
            "identifier" => {
                if let Ok(t) = child.utf8_text(src) {
                    used_names.insert(t.to_string());
                }
            }
            "marker_annotation" | "annotation" => {
                if let Some(name) = child.child_by_field_name("name") {
                    if let Ok(t) = name.utf8_text(src) {
                        let simple = t.rsplit('.').next().unwrap_or(t);
                        annotations.insert(simple.to_string());
                        referenced.insert(simple.to_string());
                    }
                }
            }
            _ => {}
        }
        collect_references(child, src, referenced, used_names, annotations);
    }
}

fn collect_public_methods(node: Node, src: &[u8], out: &mut Vec<MemberDecl>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "method_declaration" && is_exported_method(child, src) {
            if let Some(name) = child.child_by_field_name("name").and_then(|n| n.utf8_text(src).ok())
            {
                if name != "main" {
                    out.push(MemberDecl { name: name.to_string(), line: child.start_position().row as u32 + 1 });
                }
            }
        }
        collect_public_methods(child, src, out);
    }
}

fn is_exported_method(node: Node, src: &[u8]) -> bool {
    let visible = has_modifier(node, src, "public") || has_modifier(node, src, "protected");
    visible && !has_annotation(node, src, "Override")
}

fn has_annotation(node: Node, src: &[u8], name: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut inner = child.walk();
            for m in child.children(&mut inner) {
                if matches!(m.kind(), "marker_annotation" | "annotation") {
                    if let Some(n) = m.child_by_field_name("name").and_then(|x| x.utf8_text(src).ok())
                    {
                        if n.rsplit('.').next().unwrap_or(n) == name {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
