use tree_sitter::{Parser, Tree};

pub fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .ok()?;
    parser.parse(source, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_sexp() {
        let src = r#"
class A {
    void f(int x, String... rest) {
        if (x > 0 && x < 10) {
            for (int i = 0; i < x; i++) {
                System.out.println(i);
            }
        } else if (x < 0) {
            while (x < 0) { x++; }
        } else {
            switch (x) {
                case 1: break;
                case 2: return;
                default: break;
            }
        }
    }
    Runnable g = () -> { if (true) {} };
}
"#;
        let tree = parse(src).unwrap();
        println!("{}", tree.root_node().to_sexp());
    }
}
