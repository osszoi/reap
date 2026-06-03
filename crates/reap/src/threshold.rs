use crate::config::Config;
use crate::types::{Severity, Source};
use crate::JavaResult;

fn plural(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n > 1 { "s" } else { "" })
}

pub fn check_thresholds(config: &Config, result: &JavaResult) -> (bool, Vec<String>) {
    let reasons: Vec<String> = config
        .fail_on
        .iter()
        .filter_map(|t| check_one(t, config, result))
        .collect();
    (reasons.is_empty(), reasons)
}

fn check_one(target: &str, config: &Config, result: &JavaResult) -> Option<String> {
    match target {
        "nullpointers" => {
            let n = result
                .findings
                .iter()
                .filter(|f| f.source == Source::Spotbugs && f.rule.starts_with("NP_"))
                .count();
            (n > 0).then(|| format!("{} found", plural(n, "null pointer path")))
        }
        "bugs" => {
            let n = result
                .findings
                .iter()
                .filter(|f| f.source == Source::Spotbugs && f.severity != Severity::Low)
                .count();
            (n > 0).then(|| plural(n, "SpotBugs finding"))
        }
        "hotspots" => {
            if config.max_hotspot_score.is_infinite() {
                return None;
            }
            let n = result.hotspots.iter().filter(|h| h.score > config.max_hotspot_score).count();
            (n > 0).then(|| {
                format!("{} exceed hotspot score {}", plural(n, "file"), config.max_hotspot_score)
            })
        }
        "complexity" => {
            let n = result.complexity.len();
            (n > 0).then(|| format!("{} over threshold", plural(n, "complex function")))
        }
        "large-functions" => {
            let n = result.large.len();
            (n > 0).then(|| plural(n, "large function"))
        }
        "circular" => {
            let n = result.cycles.len();
            (n > 0).then(|| plural(n, "circular dependency"))
        }
        "unused-files" => {
            let n = result.unused_files.len();
            (n > 0).then(|| plural(n, "unused file"))
        }
        "unused-exports" => {
            let n = result.unused_exports.len();
            (n > 0).then(|| plural(n, "unused export"))
        }
        "duplicates" => {
            let n = result.clone_groups.len();
            (n > 0).then(|| plural(n, "duplicate block"))
        }
        "all" => {
            let n = result
                .findings
                .iter()
                .filter(|f| matches!(f.severity, Severity::Critical | Severity::High))
                .count();
            (n > 0).then(|| plural(n, "critical/high finding"))
        }
        _ => None,
    }
}
