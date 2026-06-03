#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Pmd,
    Spotbugs,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub file: String,
    pub line: String,
    pub rule: String,
    pub ruleset: String,
    pub severity: Severity,
    pub message: String,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    Stable,
    Accelerating,
    Cooling,
}

#[derive(Debug, Clone)]
pub struct Hotspot {
    pub file: String,
    pub score: f64,
    pub weighted_commits: f64,
    pub total_commits: usize,
    pub loc: usize,
    pub churn_lines: u64,
    pub density: f64,
    pub fan_in: usize,
    pub trend: Trend,
}

#[derive(Debug, Clone)]
pub struct CircularDependency {
    pub files: Vec<String>,
    pub cross_package: bool,
}

#[derive(Debug, Clone)]
pub struct UnusedFile {
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct UnusedExport {
    pub path: String,
    pub name: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct CloneInstance {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone)]
pub struct CloneGroup {
    pub instances: Vec<CloneInstance>,
    pub token_count: usize,
    pub line_count: u32,
}

#[derive(Debug, Clone)]
pub struct CloneFamily {
    pub files: Vec<String>,
    pub group_count: usize,
    pub total_lines: u32,
    pub suggestion: String,
}

#[derive(Debug, Clone)]
pub struct RefactoringTarget {
    pub path: String,
    pub priority: f64,
    pub efficiency: f64,
    pub recommendation: String,
    pub category: String,
    pub effort: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exceeded {
    Cyclomatic,
    Cognitive,
    Both,
}

#[derive(Debug, Clone)]
pub struct ComplexityViolation {
    pub path: String,
    pub name: String,
    pub line: u32,
    pub cyclomatic: u16,
    pub cognitive: u16,
    pub line_count: u32,
    pub exceeded: Exceeded,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct LargeFunction {
    pub path: String,
    pub name: String,
    pub line: u32,
    pub line_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subcommand {
    All,
    Hotspots,
    DeadCode,
    Complexity,
    Bugs,
    Circular,
    UnusedFiles,
    Deps,
    Duplicates,
    UnusedExports,
    Targets,
}
