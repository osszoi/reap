use crate::churn::collect_churn;
use crate::java::graph::ModuleGraph;
use crate::types::{Hotspot, Trend};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Clone)]
struct FileMetrics {
    density: f64,
    fan_in: usize,
    loc: usize,
}

struct Row {
    path: String,
    weighted: f64,
    commits: usize,
    churn_lines: u64,
    trend: Trend,
    metrics: FileMetrics,
}

pub fn analyze_hotspots(
    cwd: &Path,
    graph: Option<&ModuleGraph>,
    min_commits: usize,
) -> std::io::Result<Vec<Hotspot>> {
    let churn = collect_churn(cwd)?;
    let metrics = graph.map(|g| index_metrics(g, cwd)).unwrap_or_default();

    let rows: Vec<Row> = churn
        .into_iter()
        .map(|(path, c)| {
            let metrics = metrics.get(&path).cloned().unwrap_or_else(|| FileMetrics {
                density: 0.0,
                fan_in: 0,
                loc: count_lines(&cwd.join(&path)),
            });
            Row {
                path,
                weighted: c.weighted_commits,
                commits: c.total_commits(),
                churn_lines: c.added + c.deleted,
                trend: c.trend(),
                metrics,
            }
        })
        .filter(|r| r.commits >= min_commits)
        .collect();

    let max_weighted = rows.iter().map(|r| r.weighted).fold(0.0_f64, f64::max);
    let max_density = rows.iter().map(|r| r.metrics.density).fold(0.0_f64, f64::max);

    let mut hotspots: Vec<Hotspot> = rows
        .iter()
        .map(|r| {
            let norm_churn = if max_weighted > 0.0 { r.weighted / max_weighted } else { 0.0 };
            let norm_density =
                if max_density > 0.0 { r.metrics.density / max_density } else { 0.0 };
            Hotspot {
                file: r.path.clone(),
                score: round1(norm_churn * norm_density * 100.0),
                weighted_commits: round2(r.weighted),
                total_commits: r.commits,
                loc: r.metrics.loc,
                churn_lines: r.churn_lines,
                density: round2(r.metrics.density),
                fan_in: r.metrics.fan_in,
                trend: r.trend,
            }
        })
        .collect();

    hotspots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(hotspots)
}

fn index_metrics(graph: &ModuleGraph, cwd: &Path) -> HashMap<String, FileMetrics> {
    graph
        .files
        .iter()
        .enumerate()
        .map(|(id, file)| {
            let rel = file.path.strip_prefix(cwd).unwrap_or(&file.path);
            let total_cyclomatic: u32 = file.functions.iter().map(|f| f.cyclomatic as u32).sum();
            let density = if file.line_count > 0 {
                total_cyclomatic as f64 / file.line_count as f64
            } else {
                0.0
            };
            (
                rel.to_string_lossy().into_owned(),
                FileMetrics { density, fan_in: graph.fan_in(id), loc: file.line_count as usize },
            )
        })
        .collect()
}

fn count_lines(path: &Path) -> usize {
    fs::read_to_string(path).map(|s| s.split('\n').count()).unwrap_or(0)
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
