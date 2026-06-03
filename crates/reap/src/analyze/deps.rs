use std::path::Path;
use std::process::Command;

#[derive(Default)]
pub struct DependencyReport {
    pub unused: Vec<String>,
    pub undeclared: Vec<String>,
}

enum Mode {
    None,
    Unused,
    Undeclared,
}

pub fn analyze(cwd: &Path) -> std::io::Result<DependencyReport> {
    let out = Command::new("mvn")
        .args(["dependency:analyze", "--no-transfer-progress", "-B"])
        .current_dir(cwd)
        .output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(parse(&text))
}

fn parse(text: &str) -> DependencyReport {
    let mut report = DependencyReport::default();
    let mut mode = Mode::None;

    for line in text.lines() {
        let l = strip_log_prefix(line);
        if l.ends_with("Used undeclared dependencies found:") {
            mode = Mode::Undeclared;
            continue;
        }
        if l.ends_with("Unused declared dependencies found:") {
            mode = Mode::Unused;
            continue;
        }
        if let Some(coords) = gav_coords(l) {
            match mode {
                Mode::Unused => push_unique(&mut report.unused, coords),
                Mode::Undeclared => push_unique(&mut report.undeclared, coords),
                Mode::None => {}
            }
        } else {
            mode = Mode::None;
        }
    }
    report
}

fn strip_log_prefix(line: &str) -> &str {
    line.trim()
        .trim_start_matches("[WARNING]")
        .trim_start_matches("[INFO]")
        .trim()
}

fn gav_coords(line: &str) -> Option<String> {
    if line.is_empty() || line.contains(char::is_whitespace) {
        return None;
    }
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() >= 4 {
        Some(format!("{}:{}", parts[0], parts[1]))
    } else {
        None
    }
}

fn push_unique(v: &mut Vec<String>, item: String) {
    if !v.contains(&item) {
        v.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maven_output() {
        let sample = "\
[INFO] --- maven-dependency-plugin:3.6.0:analyze ---
[WARNING] Used undeclared dependencies found:
[WARNING]    com.google.guava:guava:jar:31.1-jre:compile
[WARNING] Unused declared dependencies found:
[WARNING]    org.apache.commons:commons-lang3:jar:3.12.0:compile
[WARNING]    org.projectlombok:lombok:jar:1.18.30:provided
[INFO] BUILD SUCCESS
";
        let r = parse(sample);
        assert_eq!(r.undeclared, vec!["com.google.guava:guava"]);
        assert_eq!(
            r.unused,
            vec!["org.apache.commons:commons-lang3", "org.projectlombok:lombok"]
        );
    }
}
