use crate::types::Subcommand;
use std::path::PathBuf;

pub struct Config {
    pub sub: Subcommand,
    pub fail_on: Vec<String>,
    pub max_complexity: u32,
    pub max_cognitive: u32,
    pub max_hotspot_score: f64,
    pub min_commits: usize,
    pub min_tokens: usize,
    pub min_lines: u32,
    pub no_compile: bool,
    pub verbose: bool,
    pub top: usize,
    pub cwd: PathBuf,
    pub legend: bool,
    pub skip_patterns: Vec<String>,
    pub compare_against: Option<String>,
}

pub const DEFAULT_MAX_COMPLEXITY: u32 = 20;
pub const DEFAULT_MAX_COGNITIVE: u32 = 15;
pub const DEFAULT_TOP: usize = 20;
pub const DEFAULT_MIN_COMMITS: usize = 1;

pub fn parse_fail_on(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}
