use crate::java::parse::parse;
use tree_sitter::Node;

#[derive(Debug, Clone)]
pub struct FunctionMetrics {
    pub name: String,
    pub line: u32,
    pub col: u32,
    pub cyclomatic: u16,
    pub cognitive: u16,
    pub line_count: u32,
    pub param_count: u8,
}

pub fn extract_functions(source: &str) -> Vec<FunctionMetrics> {
    let Some(tree) = parse(source) else {
        return Vec::new();
    };
    functions_in(tree.root_node(), source.as_bytes())
}

pub fn functions_in(root: Node, src: &[u8]) -> Vec<FunctionMetrics> {
    let mut funcs = Vec::new();
    collect_functions(root, src, &mut funcs);
    funcs
}

fn is_function(kind: &str) -> bool {
    matches!(
        kind,
        "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "lambda_expression"
    )
}

fn collect_functions(node: Node, src: &[u8], out: &mut Vec<FunctionMetrics>) {
    if is_function(node.kind()) {
        out.push(metrics_of(node, src));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, src, out);
    }
}

fn metrics_of(func: Node, src: &[u8]) -> FunctionMetrics {
    let start = func.start_position();
    let end = func.end_position();
    FunctionMetrics {
        name: function_name(func, src),
        line: start.row as u32 + 1,
        col: start.column as u32,
        cyclomatic: cyclomatic_of(func),
        cognitive: cognitive_of(func),
        line_count: (end.row - start.row) as u32 + 1,
        param_count: param_count(func),
    }
}

fn function_name(func: Node, src: &[u8]) -> String {
    if func.kind() == "lambda_expression" {
        return "<lambda>".to_string();
    }
    func.child_by_field_name("name")
        .and_then(|n| n.utf8_text(src).ok())
        .unwrap_or("<anonymous>")
        .to_string()
}

fn param_count(func: Node) -> u8 {
    let params = func.child_by_field_name("parameters");
    let Some(params) = params else { return 0 };
    let n = match params.kind() {
        "formal_parameters" => count_formal_parameters(params),
        "inferred_parameters" => params.named_child_count(),
        "identifier" => 1,
        _ => 0,
    };
    n.min(u8::MAX as usize) as u8
}

fn count_formal_parameters(fp: Node) -> usize {
    let mut cursor = fp.walk();
    fp.named_children(&mut cursor)
        .filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter"))
        .count()
}

// --- cyclomatic (McCabe), base 1 ---

fn cyclomatic_of(func: Node) -> u16 {
    let c = 1 + walk_cyclo(func);
    c.min(u16::MAX as u32) as u16
}

fn walk_cyclo(node: Node) -> u32 {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if is_function(child.kind()) {
            continue;
        }
        count += cyclo_increment(child);
        count += walk_cyclo(child);
    }
    count
}

fn cyclo_increment(node: Node) -> u32 {
    match node.kind() {
        "if_statement" | "for_statement" | "enhanced_for_statement" | "while_statement"
        | "do_statement" | "catch_clause" | "ternary_expression" => 1,
        "switch_label" => (node.named_child_count() > 0) as u32,
        "&&" | "||" => 1,
        _ => 0,
    }
}

// --- cognitive (SonarSource), base 0, with nesting penalty ---

fn cognitive_of(func: Node) -> u16 {
    let mut total: u32 = 0;
    visit_cog(func, 0, &mut total);
    total.min(u16::MAX as u32) as u16
}

fn visit_cog(node: Node, nesting: u32, total: &mut u32) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            k if is_function(k) => {}
            "if_statement" => visit_if_chain(child, nesting, total),
            "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement" => {
                *total += 1 + nesting;
                visit_with_body_nested(child, nesting, total);
            }
            "switch_expression" | "catch_clause" => {
                *total += 1 + nesting;
                visit_with_body_nested(child, nesting, total);
            }
            "ternary_expression" => {
                *total += 1 + nesting;
                visit_ternary(child, nesting, total);
            }
            "break_statement" | "continue_statement" => {
                if has_label(child) {
                    *total += 1;
                }
            }
            "binary_expression" => {
                if let Some(op) = logical_op(child) {
                    if !parent_is_same_logical(child, op) {
                        *total += 1;
                    }
                }
                visit_cog(child, nesting, total);
            }
            _ => visit_cog(child, nesting, total),
        }
    }
}

fn visit_if_chain(node: Node, nesting: u32, total: &mut u32) {
    *total += 1 + nesting;
    if let Some(cond) = node.child_by_field_name("condition") {
        visit_cog(cond, nesting, total);
    }
    if let Some(cons) = node.child_by_field_name("consequence") {
        visit_cog(cons, nesting + 1, total);
    }
    let mut current = node;
    while let Some(alt) = current.child_by_field_name("alternative") {
        if alt.kind() == "if_statement" {
            *total += 1;
            if let Some(cond) = alt.child_by_field_name("condition") {
                visit_cog(cond, nesting, total);
            }
            if let Some(cons) = alt.child_by_field_name("consequence") {
                visit_cog(cons, nesting + 1, total);
            }
            current = alt;
        } else {
            *total += 1;
            visit_cog(alt, nesting + 1, total);
            break;
        }
    }
}

fn visit_with_body_nested(node: Node, nesting: u32, total: &mut u32) {
    let body_id = node.child_by_field_name("body").map(|b| b.id());
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let inner = if Some(child.id()) == body_id { nesting + 1 } else { nesting };
        visit_cog(child, inner, total);
    }
}

fn visit_ternary(node: Node, nesting: u32, total: &mut u32) {
    if let Some(cond) = node.child_by_field_name("condition") {
        visit_cog(cond, nesting, total);
    }
    for field in ["consequence", "alternative"] {
        if let Some(branch) = node.child_by_field_name(field) {
            visit_cog(branch, nesting + 1, total);
        }
    }
}

fn logical_op(node: Node) -> Option<&'static str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "&&" => return Some("&&"),
            "||" => return Some("||"),
            _ => {}
        }
    }
    None
}

fn parent_is_same_logical(node: Node, op: &str) -> bool {
    node.parent()
        .filter(|p| p.kind() == "binary_expression")
        .and_then(logical_op)
        .map(|parent_op| parent_op == op)
        .unwrap_or(false)
}

fn has_label(node: Node) -> bool {
    let mut cursor = node.walk();
    let mut labeled = false;
    for c in node.named_children(&mut cursor) {
        if c.kind() == "identifier" {
            labeled = true;
            break;
        }
    }
    labeled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn by_name<'a>(fns: &'a [FunctionMetrics], name: &str) -> &'a FunctionMetrics {
        fns.iter().find(|f| f.name == name).expect("function not found")
    }

    #[test]
    fn cyclomatic_counts_decision_points() {
        // if + (&& , ||) + for + while + 2 case labels + catch = 1 base + 8 = 9
        let src = r#"
class A {
  void complex(int x, int y) {
    if (x > 0 && y > 0 || x == y) {
      for (int i = 0; i < x; i++) {
        while (y > 0) { y--; }
      }
    }
    switch (x) { case 1: break; case 2: break; default: break; }
    try { } catch (Exception e) { }
  }
}
"#;
        let fns = extract_functions(src);
        let f = by_name(&fns, "complex");
        assert_eq!(f.cyclomatic, 9, "cyclomatic");
        assert_eq!(f.param_count, 2);
    }

    #[test]
    fn cognitive_applies_nesting_penalty() {
        // if (1) -> if (2) -> if (3) = 1+2+3 = 6
        let src = r#"
class A {
  void nested(int x) {
    if (x > 0) {
      if (x > 1) {
        if (x > 2) { }
      }
    }
  }
}
"#;
        let fns = extract_functions(src);
        let f = by_name(&fns, "nested");
        assert_eq!(f.cognitive, 6, "cognitive nesting");
    }

    #[test]
    fn line_count_and_lambda() {
        let src = "class A {\n  Runnable r = () -> {\n    System.out.println(1);\n  };\n}\n";
        let fns = extract_functions(src);
        let lambda = by_name(&fns, "<lambda>");
        assert_eq!(lambda.line_count, 3);
    }
}
