use crate::types::{
    CircularDependency, CloneFamily, CloneGroup, ComplexityViolation, Exceeded, Finding, Hotspot,
    LargeFunction, RefactoringTarget, Severity, Trend, UnusedExport, UnusedFile,
};
use owo_colors::OwoColorize;
use std::path::Path;

pub fn print_header(version: &str, name: &str, lang: &str) {
    println!();
    print!("{}", "  reap".bold().cyan());
    print!("{}", format!(" v{version}  ·  ").dimmed());
    print!("{}", name.white());
    print!("{}", format!("  ·  {lang}").dimmed());
    println!("\n");
}

pub fn print_section(title: &str, count: &str) {
    println!("{}{}{}", "  ⬡ ".cyan().bold(), title.cyan().bold(), format!("  {count}").dimmed());
    println!("{}", format!("  {}", "─".repeat(56)).dimmed());
}

pub fn print_legend(text: &str) {
    println!("{}", format!("  {text}").dimmed());
}

pub fn print_section_count(title: &str, count: usize) {
    let cnt = if count == 0 {
        count.to_string().green().to_string()
    } else {
        count.to_string().yellow().to_string()
    };
    println!("{}{}  {}", "  ⬡ ".cyan().bold(), title.cyan().bold(), cnt);
    println!("{}", format!("  {}", "─".repeat(56)).dimmed());
}

fn trend_symbol(t: Trend) -> String {
    match t {
        Trend::Accelerating => "▲".red().to_string(),
        Trend::Cooling => "▼".green().to_string(),
        Trend::Stable => "─".dimmed().to_string(),
    }
}

pub fn print_hotspots(hotspots: &[Hotspot], top: usize, cwd: &Path) {
    if hotspots.is_empty() {
        println!("{}", "    no hotspots found".dimmed());
        println!();
        return;
    }

    for h in hotspots.iter().take(top) {
        let score_str = format!("{:>5.1}", h.score);
        let score_colored = if h.score >= 70.0 {
            score_str.red().bold().to_string()
        } else if h.score >= 30.0 {
            score_str.yellow().to_string()
        } else {
            score_str.green().to_string()
        };

        println!("  {}  {}  {}", score_colored, trend_symbol(h.trend), h.file.white());
        let meta = format!(
            "{:>3} commits  {:>5} churn  {:.2} density  {:>2} fan-in  {}",
            h.total_commits,
            h.churn_lines,
            h.density,
            h.fan_in,
            trend_label(h.trend),
        );
        println!("          {}", meta.dimmed());
    }
    println!();
    let _ = cwd;
}

fn trend_label(t: Trend) -> String {
    match t {
        Trend::Accelerating => format!("{} accelerating", "▲".red()),
        Trend::Cooling => format!("{} cooling", "▼".green()),
        Trend::Stable => format!("{} stable", "─".dimmed()),
    }
}

pub fn print_circular(cycles: &[CircularDependency], top: usize) {
    if cycles.is_empty() {
        println!("{}", "    no circular dependencies".dimmed());
        println!();
        return;
    }
    for c in cycles.iter().take(top) {
        let tag = if c.cross_package {
            " (cross-package)".dimmed().to_string()
        } else {
            String::new()
        };
        let chain = c
            .files
            .iter()
            .map(|f| basename(f))
            .collect::<Vec<_>>()
            .join(&format!(" {} ", "→".dimmed()));
        println!("  {} files{}", c.files.len(), tag);
        println!("    {chain}");
    }
    println!();
}

pub fn print_clone_groups(groups: &[CloneGroup], top: usize) {
    if groups.is_empty() {
        println!("{}", "    no duplicate code".dimmed());
        println!();
        return;
    }
    for g in groups.iter().take(top) {
        let header = format!("  {:>4} lines  {} instances", g.line_count, g.instances.len());
        println!("{}", header.yellow());
        for inst in &g.instances {
            println!(
                "    {}",
                format!("{}:{}-{}", inst.file, inst.start_line, inst.end_line).dimmed()
            );
        }
    }
    println!();
}

pub fn print_clone_families(families: &[CloneFamily], top: usize) {
    if families.is_empty() {
        println!("{}", "    no clone families".dimmed());
        println!();
        return;
    }
    for f in families.iter().take(top) {
        println!(
            "  {} groups, {} lines across {}",
            f.group_count.to_string().bold(),
            f.total_lines.to_string().bold(),
            f.files.join(", ")
        );
        println!("    {} {}", "→".yellow(), f.suggestion);
    }
    println!();
}

pub fn print_unused_exports(items: &[UnusedExport], top: usize) {
    if items.is_empty() {
        println!("{}", "    no unused exports".dimmed());
        println!();
        return;
    }
    let mut shown = 0;
    for (path, group) in group_by_path(items, |e| e.path.as_str()) {
        if shown >= top {
            break;
        }
        println!("  {}", path.white());
        for e in group {
            if shown >= top {
                break;
            }
            shown += 1;
            println!("    {} {}", format!(":{}", e.line).dimmed(), e.name.bold());
        }
    }
    println!();
}

pub fn print_targets(targets: &[RefactoringTarget], top: usize) {
    if targets.is_empty() {
        println!("{}", "    no refactoring targets".dimmed());
        println!();
        return;
    }
    for t in targets.iter().take(top) {
        let eff = format!("{:>5.1}", t.efficiency);
        let eff_colored = if t.efficiency >= 40.0 {
            eff.green().to_string()
        } else if t.efficiency >= 20.0 {
            eff.yellow().to_string()
        } else {
            eff.dimmed().to_string()
        };
        println!("  {}  {}    {}", eff_colored, format!("pri:{:.1}", t.priority).dimmed(), t.path.white());
        println!(
            "         {} · effort:{} · confidence:{}  {}",
            t.category.cyan(),
            t.effort,
            t.confidence,
            t.recommendation.dimmed()
        );
    }
    println!();
}

pub fn print_dependencies(items: &[String]) {
    if items.is_empty() {
        println!("{}", "    none".dimmed());
        println!();
        return;
    }
    for item in items {
        println!("  {}", item.white());
    }
    println!();
}

pub fn print_unused_files(files: &[UnusedFile], top: usize) {
    if files.is_empty() {
        println!("{}", "    no unused files".dimmed());
        println!();
        return;
    }
    for f in files.iter().take(top) {
        println!("  {}", f.path.white());
    }
    println!();
}

fn group_by_path<'a, T>(items: &'a [T], key: impl Fn(&T) -> &str) -> Vec<(&'a str, Vec<&'a T>)> {
    let mut groups: Vec<(&str, Vec<&T>)> = Vec::new();
    for item in items {
        let path = key(item);
        match groups.iter_mut().find(|(p, _)| *p == path) {
            Some((_, v)) => v.push(item),
            None => groups.push((path, vec![item])),
        }
    }
    groups
}

fn severity_tag(s: Severity) -> String {
    match s {
        Severity::Critical => " CRITICAL".red().bold().to_string(),
        Severity::High => " HIGH".yellow().bold().to_string(),
        _ => String::new(),
    }
}

pub fn print_complexity(
    violations: &[ComplexityViolation],
    top: usize,
    max_cyclomatic: u16,
    max_cognitive: u16,
) {
    if violations.is_empty() {
        println!("{}", "    no functions over threshold".dimmed());
        println!();
        return;
    }

    let mut shown = 0;
    for (path, items) in group_by_path(violations, |v| v.path.as_str()) {
        if shown >= top {
            break;
        }
        println!("  {}", path.white());
        for v in items {
            if shown >= top {
                break;
            }
            shown += 1;
            let exceeded = matches!(v.exceeded, Exceeded::Both);
            let _ = exceeded;
            println!(
                "    {} {}{}",
                format!(":{}", v.line).dimmed(),
                v.name.bold(),
                severity_tag(v.severity)
            );
            let cyc = metric(v.cyclomatic as u32, v.cyclomatic > max_cyclomatic);
            let cog = metric(v.cognitive as u32, v.cognitive > max_cognitive);
            let lines = format!("{:>3} lines", v.line_count).dimmed().to_string();
            println!("         {cyc} cyclomatic   {cog} cognitive   {lines}");
        }
    }
    println!();
}

pub fn print_large_functions(entries: &[LargeFunction], top: usize) {
    if entries.is_empty() {
        println!("{}", "    no large functions".dimmed());
        println!();
        return;
    }

    let mut shown = 0;
    for (path, items) in group_by_path(entries, |e| e.path.as_str()) {
        if shown >= top {
            break;
        }
        println!("  {}", path.white());
        for e in items {
            if shown >= top {
                break;
            }
            shown += 1;
            let count = format!("{:>3} lines", e.line_count).red().bold().to_string();
            println!("    {} {}  {}", format!(":{}", e.line).dimmed(), e.name.bold(), count);
        }
    }
    println!();
}

fn metric(value: u32, over: bool) -> String {
    let s = format!("{value:>3}");
    if over {
        s.red().bold().to_string()
    } else {
        s.dimmed().to_string()
    }
}

pub fn print_findings(findings: &[Finding], cwd: &Path) {
    if findings.is_empty() {
        println!("{}", "    no findings".dimmed());
        println!();
        return;
    }

    for f in findings {
        let icon = match f.severity {
            Severity::Critical | Severity::High => "●",
            _ => "◦",
        };
        let label = format!("{:<8}", severity_label(f.severity));
        let sev = colorize_severity(f.severity, &format!("{icon} {label}"));
        let loc = format!("    {}:{}", shorten(&f.file, cwd), f.line).dimmed().to_string();
        println!("  {}  {}", sev, f.rule.dimmed());
        println!("{}  {}", loc, f.message);
        println!();
    }
}

pub fn print_summary(passed: bool, reasons: &[String]) {
    println!("{}", format!("  {}", "─".repeat(56)).dimmed());
    if passed {
        println!("{}", "  ✔  passed — no violations above threshold".green().bold());
    } else {
        for r in reasons {
            println!("{}", format!("  ✖  {r}").red().bold());
        }
    }
    println!();
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Critical => "critical",
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
    }
}

fn colorize_severity(s: Severity, text: &str) -> String {
    match s {
        Severity::Critical => text.red().bold().to_string(),
        Severity::High => text.yellow().bold().to_string(),
        Severity::Medium => text.yellow().to_string(),
        Severity::Low => text.blue().to_string(),
    }
}

fn basename(p: &str) -> String {
    p.rsplit('/').next().unwrap_or(p).to_string()
}

fn shorten(p: &str, cwd: &Path) -> String {
    let prefix = format!("{}/", cwd.display());
    p.strip_prefix(&prefix).unwrap_or(p).to_string()
}
