use crate::java::graph::ModuleGraph;
use crate::types::{CircularDependency, Hotspot, RefactoringTarget, Trend, UnusedExport};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const COGNITIVE_EXTRACTION: u16 = 30;

struct Thresholds {
    fan_in_p95: f64,
    fan_in_p75: f64,
    fan_in_p25: f64,
    fan_out_p95: f64,
    fan_out_p90: f64,
}

struct Metrics {
    rel: String,
    fan_in: usize,
    fan_out: usize,
    density: f64,
    function_count: usize,
    lines: u32,
    dead_ratio: f64,
    value_exports: usize,
    max_cognitive: u16,
    is_circular: bool,
    is_root: bool,
    hotspot: f64,
    trend: Trend,
}

pub fn collect(
    graph: &ModuleGraph,
    hotspots: &[Hotspot],
    cycles: &[CircularDependency],
    unused_exports: &[UnusedExport],
    cwd: &Path,
) -> Vec<RefactoringTarget> {
    let hotspot_by: HashMap<&str, (f64, Trend)> =
        hotspots.iter().map(|h| (h.file.as_str(), (h.score, h.trend))).collect();
    let circular: HashSet<&str> =
        cycles.iter().flat_map(|c| c.files.iter().map(|s| s.as_str())).collect();
    let mut unused_by: HashMap<&str, usize> = HashMap::new();
    for u in unused_exports {
        *unused_by.entry(u.path.as_str()).or_insert(0) += 1;
    }

    let all: Vec<Metrics> = graph
        .files
        .iter()
        .enumerate()
        .map(|(id, f)| {
            let rel = f.path.strip_prefix(cwd).unwrap_or(&f.path).to_string_lossy().into_owned();
            let total_cyc: u32 = f.functions.iter().map(|x| x.cyclomatic as u32).sum();
            let density = if f.line_count > 0 { total_cyc as f64 / f.line_count as f64 } else { 0.0 };
            let value_exports = f.exports.len();
            let unused = unused_by.get(rel.as_str()).copied().unwrap_or(0);
            let dead_ratio = if value_exports > 0 { unused as f64 / value_exports as f64 } else { 0.0 };
            let (hotspot, trend) = hotspot_by.get(rel.as_str()).copied().unwrap_or((0.0, Trend::Stable));
            let is_circular = circular.contains(rel.as_str());
            Metrics {
                rel,
                fan_in: graph.fan_in(id),
                fan_out: graph.fan_out(id),
                density,
                function_count: f.functions.len(),
                lines: f.line_count,
                dead_ratio,
                value_exports,
                max_cognitive: f.functions.iter().map(|x| x.cognitive).max().unwrap_or(0),
                is_circular,
                is_root: graph.is_root[id],
                hotspot,
                trend,
            }
        })
        .collect();

    let th = thresholds(&all);

    let mut targets: Vec<RefactoringTarget> = all
        .iter()
        .filter_map(|m| match_target(m, &th))
        .collect();

    targets.sort_by(|a, b| {
        b.efficiency
            .partial_cmp(&a.efficiency)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.path.cmp(&b.path))
    });
    targets
}

fn match_target(m: &Metrics, th: &Thresholds) -> Option<RefactoringTarget> {
    let (category, recommendation, confidence) = rule(m, th)?;
    let priority = priority(m, th);
    let effort_n = effort(m, th);
    let efficiency = round1(priority / effort_n as f64);
    Some(RefactoringTarget {
        path: m.rel.clone(),
        priority,
        efficiency,
        recommendation: recommendation.to_string(),
        category: category.to_string(),
        effort: effort_label(effort_n).to_string(),
        confidence: confidence.to_string(),
    })
}

fn rule(m: &Metrics, th: &Thresholds) -> Option<(&'static str, String, &'static str)> {
    if m.hotspot >= 50.0 && m.trend == Trend::Accelerating && m.density > 0.5 {
        return Some(("churn+complexity", "Actively-changing file with growing complexity — stabilize before adding features".into(), "low"));
    }
    if m.is_circular && m.fan_in >= 5 {
        return Some(("circular dependency", "Break the import cycle — high fan-in makes it risky".into(), "high"));
    }
    if m.density > 0.3 && (m.fan_in as f64 >= th.fan_in_p95 || (m.fan_in as f64 >= th.fan_in_p75 && m.function_count >= 5)) {
        return Some(("high impact", "Split this high-impact, complex file".into(), "medium"));
    }
    if m.dead_ratio >= 0.5 && m.value_exports >= 3 {
        let pct = (m.dead_ratio * 100.0).round() as u32;
        return Some(("dead code", format!("Remove unused public API to reduce surface area ({pct}% dead)"), "high"));
    }
    if m.max_cognitive >= COGNITIVE_EXTRACTION {
        return Some(("complexity", "Extract complex functions into smaller units".into(), "high"));
    }
    if !m.is_root && m.fan_out as f64 >= th.fan_out_p90 {
        return Some(("coupling", "High outgoing coupling — extract dependencies".into(), "medium"));
    }
    if m.is_circular {
        return Some(("circular dependency", "Break the import cycle".into(), "high"));
    }
    None
}

fn priority(m: &Metrics, th: &Thresholds) -> f64 {
    let density_norm = m.density.min(1.0);
    let fan_in_norm = (m.fan_in as f64 / th.fan_in_p95).min(1.0);
    let fan_out_norm = (m.fan_out as f64 / th.fan_out_p95).min(1.0);
    let hotspot_boost = m.hotspot / 100.0;
    let p = density_norm * 30.0
        + hotspot_boost * 25.0
        + m.dead_ratio * 20.0
        + fan_in_norm * 15.0
        + fan_out_norm * 10.0;
    round1(p.clamp(0.0, 100.0))
}

fn effort(m: &Metrics, th: &Thresholds) -> u8 {
    if m.lines >= 500 || m.fan_in as f64 >= th.fan_in_p95 || (m.function_count >= 15 && m.density > 0.5) {
        3
    } else if m.lines < 100 && m.function_count <= 3 && (m.fan_in as f64) < th.fan_in_p25 {
        1
    } else {
        2
    }
}

fn effort_label(n: u8) -> &'static str {
    match n {
        1 => "low",
        3 => "high",
        _ => "medium",
    }
}

fn thresholds(all: &[Metrics]) -> Thresholds {
    let mut fan_ins: Vec<usize> = all.iter().map(|m| m.fan_in).collect();
    let mut fan_outs: Vec<usize> = all.iter().map(|m| m.fan_out).collect();
    fan_ins.sort_unstable();
    fan_outs.sort_unstable();
    Thresholds {
        fan_in_p95: percentile(&fan_ins, 0.95).max(5.0),
        fan_in_p75: percentile(&fan_ins, 0.75).max(3.0),
        fan_in_p25: percentile(&fan_ins, 0.25).max(2.0),
        fan_out_p95: percentile(&fan_outs, 0.95).max(8.0),
        fan_out_p90: percentile(&fan_outs, 0.90).max(5.0),
    }
}

fn percentile(sorted: &[usize], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 * p).ceil() as usize).min(sorted.len()) - 1;
    sorted[idx] as f64
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
