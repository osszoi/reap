use crate::java::extract::FunctionMetrics;
use crate::java::model::FileInfo;
use crate::types::{ComplexityViolation, Exceeded, Severity};
use std::path::Path;

const COGNITIVE_HIGH: u16 = 25;
const COGNITIVE_CRITICAL: u16 = 40;
const CYCLOMATIC_HIGH: u16 = 30;
const CYCLOMATIC_CRITICAL: u16 = 50;

pub fn collect(
    files: &[FileInfo],
    cwd: &Path,
    max_cyclomatic: u16,
    max_cognitive: u16,
) -> Vec<ComplexityViolation> {
    let mut violations: Vec<ComplexityViolation> = files
        .iter()
        .flat_map(|f| {
            let rel = rel_path(&f.path, cwd);
            f.functions
                .iter()
                .filter_map(move |fn_| {
                    violation_for(fn_, &rel, max_cyclomatic, max_cognitive)
                })
        })
        .collect();

    violations.sort_by(|a, b| sort_key(b).cmp(&sort_key(a)));
    violations
}

fn violation_for(
    f: &FunctionMetrics,
    rel: &str,
    max_cyclomatic: u16,
    max_cognitive: u16,
) -> Option<ComplexityViolation> {
    let over_cyc = f.cyclomatic > max_cyclomatic;
    let over_cog = f.cognitive > max_cognitive;
    if !over_cyc && !over_cog {
        return None;
    }
    let exceeded = match (over_cyc, over_cog) {
        (true, true) => Exceeded::Both,
        (true, false) => Exceeded::Cyclomatic,
        (false, true) => Exceeded::Cognitive,
        _ => unreachable!(),
    };
    Some(ComplexityViolation {
        path: rel.to_string(),
        name: f.name.clone(),
        line: f.line,
        cyclomatic: f.cyclomatic,
        cognitive: f.cognitive,
        line_count: f.line_count,
        exceeded,
        severity: severity_of(f.cyclomatic, f.cognitive),
    })
}

fn severity_of(cyclomatic: u16, cognitive: u16) -> Severity {
    let cyc = if cyclomatic >= CYCLOMATIC_CRITICAL {
        2
    } else if cyclomatic >= CYCLOMATIC_HIGH {
        1
    } else {
        0
    };
    let cog = if cognitive >= COGNITIVE_CRITICAL {
        2
    } else if cognitive >= COGNITIVE_HIGH {
        1
    } else {
        0
    };
    match cyc.max(cog) {
        2 => Severity::Critical,
        1 => Severity::High,
        _ => Severity::Medium,
    }
}

fn sort_key(v: &ComplexityViolation) -> (u8, u8, u16, u16, u32) {
    let exceeded = match v.exceeded {
        Exceeded::Both => 2,
        _ => 1,
    };
    let severity = match v.severity {
        Severity::Critical => 3,
        Severity::High => 2,
        _ => 1,
    };
    (exceeded, severity, v.cyclomatic, v.cognitive, v.line_count)
}

fn rel_path(path: &Path, cwd: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).to_string_lossy().into_owned()
}
