use crate::types::Trend;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const HALF_LIFE_SECS: f64 = 90.0 * 24.0 * 3600.0;

pub struct FileChurn {
    pub weighted_commits: f64,
    pub added: u64,
    pub deleted: u64,
    pub timestamps: Vec<i64>,
}

impl FileChurn {
    pub fn total_commits(&self) -> usize {
        self.timestamps.len()
    }

    pub fn trend(&self) -> Trend {
        compute_trend(&self.timestamps)
    }
}

pub fn collect_churn(cwd: &Path) -> std::io::Result<HashMap<String, FileChurn>> {
    let out = Command::new("git")
        .args([
            "log",
            "--numstat",
            "--no-merges",
            "--pretty=format:COMMIT %H %at",
        ])
        .current_dir(cwd)
        .output()?;

    if !out.status.success() {
        return Err(std::io::Error::other("git log failed"));
    }

    let raw = String::from_utf8_lossy(&out.stdout);
    Ok(parse_git_log(&raw))
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn parse_git_log(raw: &str) -> HashMap<String, FileChurn> {
    let mut stats: HashMap<String, FileChurn> = HashMap::new();
    let now = now_secs();
    let mut ts: i64 = 0;

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("COMMIT ") {
            ts = rest.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            continue;
        }
        if ts == 0 {
            continue;
        }
        let Some((path, added, deleted)) = parse_numstat_line(line) else {
            continue;
        };
        let decay = (-(now - ts as f64) * std::f64::consts::LN_2 / HALF_LIFE_SECS).exp();
        let entry = stats.entry(path).or_insert(FileChurn {
            weighted_commits: 0.0,
            added: 0,
            deleted: 0,
            timestamps: Vec::new(),
        });
        entry.weighted_commits += decay;
        entry.added += added;
        entry.deleted += deleted;
        entry.timestamps.push(ts);
    }

    stats
}

fn parse_numstat_line(line: &str) -> Option<(String, u64, u64)> {
    if !line.contains('\t') {
        return None;
    }
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() != 3 || parts[0] == "-" || parts[0].is_empty() {
        return None;
    }
    let path = parts[2];
    if !path.ends_with(".java") {
        return None;
    }
    let added = parts[0].parse().ok()?;
    let deleted = parts[1].parse().ok()?;
    Some((path.to_string(), added, deleted))
}

fn compute_trend(timestamps: &[i64]) -> Trend {
    if timestamps.len() < 2 {
        return Trend::Stable;
    }
    let min_ts = *timestamps.iter().min().unwrap();
    let max_ts = *timestamps.iter().max().unwrap();
    if min_ts == max_ts {
        return Trend::Stable;
    }
    let midpoint = min_ts + (max_ts - min_ts) / 2;
    let recent = timestamps.iter().filter(|&&t| t > midpoint).count();
    let older = timestamps.iter().filter(|&&t| t <= midpoint).count();
    if older < 1 {
        return Trend::Stable;
    }
    let ratio = recent as f64 / older as f64;
    if ratio > 1.5 {
        Trend::Accelerating
    } else if ratio < 0.67 {
        Trend::Cooling
    } else {
        Trend::Stable
    }
}
