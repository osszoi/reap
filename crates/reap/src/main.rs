mod analyze;
mod churn;
mod config;
mod detect;
mod explain;
mod hotspots;
mod java;
mod report;
mod report_filter;
mod threshold;
mod types;

use clap::{Parser, Subcommand as ClapSubcommand};
use config::{
    parse_fail_on, Config, DEFAULT_MAX_COGNITIVE, DEFAULT_MAX_COMPLEXITY, DEFAULT_MIN_COMMITS,
    DEFAULT_TOP,
};
use detect::{detect_project, Lang};
use java::graph::ModuleGraph;
use owo_colors::OwoColorize;
use report_filter::{ChangedError, ReportFilter};
use std::path::{Path, PathBuf};
use types::{
    CircularDependency, CloneFamily, CloneGroup, ComplexityViolation, Finding, Hotspot,
    LargeFunction, RefactoringTarget, Subcommand, UnusedExport, UnusedFile,
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "reap", version, about = "Code health scanner for Java")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    common: CommonOpts,
}

#[derive(ClapSubcommand)]
enum Command {
    /// git churn × complexity ranking
    Hotspots,
    /// unused variables, imports, methods
    DeadCode,
    /// cyclomatic and cognitive complexity
    Complexity,
    /// SpotBugs bug patterns and security findings
    Bugs,
    /// import cycles between classes
    Circular,
    /// files unreachable from any entry point
    UnusedFiles,
    /// unused and undeclared Maven dependencies
    Deps,
    /// duplicate code blocks and clone families
    Dupes,
    /// public methods with no external callers
    UnusedExports,
    /// prioritized refactoring recommendations
    Targets,
    /// describe a section or metric (no scan): reap explain cyclomatic
    Explain {
        /// topic name, e.g. cyclomatic, "unused exports"
        #[arg(trailing_var_arg = true)]
        topic: Vec<String>,
    },
}

#[derive(clap::Args)]
struct CommonOpts {
    /// comma-separated: complexity,large-functions,circular,unused-files,unused-exports,duplicates,hotspots,nullpointers,bugs,all
    #[arg(long, global = true, default_value = "nullpointers")]
    fail_on: String,
    /// cyclomatic complexity threshold
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_COMPLEXITY)]
    max_complexity: u32,
    /// cognitive complexity threshold
    #[arg(long, global = true, default_value_t = DEFAULT_MAX_COGNITIVE)]
    max_cognitive: u32,
    /// hotspot score threshold (default: no limit)
    #[arg(long, global = true)]
    max_hotspot_score: Option<f64>,
    /// skip Maven compile step
    #[arg(long, global = true)]
    skip_compile: bool,
    /// show medium/low findings too
    #[arg(long, global = true)]
    verbose: bool,
    /// how many hotspots to show
    #[arg(long, global = true, default_value_t = DEFAULT_TOP)]
    top: usize,
    /// minimum commits for a file to rank as a hotspot
    #[arg(long, global = true, default_value_t = DEFAULT_MIN_COMMITS)]
    min_commits: usize,
    /// minimum token length for a duplicate clone
    #[arg(long, global = true, default_value_t = analyze::duplicates::DEFAULT_MIN_TOKENS)]
    min_tokens: usize,
    /// minimum line height for a duplicate clone
    #[arg(long, global = true, default_value_t = analyze::duplicates::DEFAULT_MIN_LINES)]
    min_lines: u32,
    /// hide the dim per-section keyword legends
    #[arg(long = "no-legend", global = true)]
    no_legend: bool,
    /// comma-separated globs to omit from the report (still analyzed): generated,legacy/old,**/dto
    #[arg(long, global = true)]
    skip_pattern: Option<String>,
    /// only report findings introduced vs <REF> (line-level), e.g. --compare-against=master
    #[arg(long, global = true)]
    compare_against: Option<String>,
}

impl Command {
    fn to_sub(opt: &Option<Command>) -> Subcommand {
        match opt {
            None => Subcommand::All,
            Some(Command::Hotspots) => Subcommand::Hotspots,
            Some(Command::DeadCode) => Subcommand::DeadCode,
            Some(Command::Complexity) => Subcommand::Complexity,
            Some(Command::Bugs) => Subcommand::Bugs,
            Some(Command::Circular) => Subcommand::Circular,
            Some(Command::UnusedFiles) => Subcommand::UnusedFiles,
            Some(Command::Deps) => Subcommand::Deps,
            Some(Command::Dupes) => Subcommand::Duplicates,
            Some(Command::UnusedExports) => Subcommand::UnusedExports,
            Some(Command::Targets) => Subcommand::Targets,
            Some(Command::Explain { .. }) => Subcommand::All,
        }
    }
}

fn build_config(sub: Subcommand, opts: &CommonOpts, cwd: PathBuf) -> Config {
    Config {
        sub,
        cwd,
        fail_on: parse_fail_on(&opts.fail_on),
        max_complexity: opts.max_complexity,
        max_cognitive: opts.max_cognitive,
        max_hotspot_score: opts.max_hotspot_score.unwrap_or(f64::INFINITY),
        min_commits: opts.min_commits,
        min_tokens: opts.min_tokens,
        min_lines: opts.min_lines,
        no_compile: opts.skip_compile,
        verbose: opts.verbose,
        top: opts.top,
        legend: !opts.no_legend,
        skip_patterns: opts.skip_pattern.as_deref().map(parse_fail_on).unwrap_or_default(),
        compare_against: opts.compare_against.clone(),
    }
}

fn main() {
    let cli = Cli::parse();

    if let Some(Command::Explain { topic }) = &cli.command {
        explain::run(&topic.join(" "));
        return;
    }

    let sub = Command::to_sub(&cli.command);
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config = build_config(sub, &cli.common, cwd.clone());

    let project = detect_project(&cwd);

    if project.lang == Lang::Unknown {
        eprintln!("  error: no pom.xml found — are you in a Java project directory?");
        std::process::exit(2);
    }

    let repo_root = report_filter::repo_root(&project.root, &project.root);
    let filter = build_filter(&config, &repo_root);

    report::print_header(VERSION, &project.name, "Java");
    if let Some(reference) = &config.compare_against {
        if filter.compare_mode() {
            println!("{}", format!("  comparing against {reference} — showing only introduced findings").dimmed());
        }
    }
    println!();

    let result = run_java(&config, &project.root, &repo_root, &filter);
    println!();

    render_results(&config, &filter, &result);

    let (passed, reasons) = threshold::check_thresholds(&config, &result);
    report::print_summary(passed, &reasons);
    if !passed {
        std::process::exit(1);
    }
}

fn build_filter(config: &Config, repo_root: &Path) -> ReportFilter {
    let changed = match &config.compare_against {
        None => None,
        Some(reference) => match report_filter::build_changed(repo_root, reference) {
            Ok(set) => Some(set),
            Err(ChangedError::NotARepo) => {
                eprintln!("{}", "  compare-against skipped — not a git repository (showing full report)".dimmed());
                None
            }
            Err(ChangedError::BadRef(msg)) => {
                eprintln!("{}", format!("  compare-against skipped — {msg} (showing full report)").dimmed());
                None
            }
        },
    };
    ReportFilter::new(&config.skip_patterns, changed)
}

struct JavaResult {
    findings: Vec<Finding>,
    hotspots: Vec<Hotspot>,
    complexity: Vec<ComplexityViolation>,
    large: Vec<LargeFunction>,
    cycles: Vec<CircularDependency>,
    unused_files: Vec<UnusedFile>,
    unused_exports: Vec<UnusedExport>,
    targets: Vec<RefactoringTarget>,
    clone_groups: Vec<CloneGroup>,
    clone_families: Vec<CloneFamily>,
    has_dupes: bool,
    deps: Option<std::io::Result<analyze::deps::DependencyReport>>,
}

fn needs_graph(sub: Subcommand) -> bool {
    matches!(
        sub,
        Subcommand::All
            | Subcommand::Hotspots
            | Subcommand::Complexity
            | Subcommand::Circular
            | Subcommand::UnusedFiles
            | Subcommand::UnusedExports
            | Subcommand::Targets
    )
}

fn fn_span(line: u32, line_count: u32) -> (u32, u32) {
    (line, line + line_count.saturating_sub(1))
}

fn run_java(config: &Config, root: &Path, repo_root: &Path, filter: &ReportFilter) -> JavaResult {
    let sub = config.sub;
    let findings: Vec<Finding> = Vec::new();

    let graph = if needs_graph(sub) {
        Some(ModuleGraph::build(java::discover::analyze_files(root)))
    } else {
        None
    };

    let want_complexity = matches!(sub, Subcommand::All | Subcommand::Complexity);
    let want_circular = matches!(sub, Subcommand::All | Subcommand::Circular);
    let want_unused_files = matches!(sub, Subcommand::All | Subcommand::UnusedFiles);
    let want_unused_exports = matches!(sub, Subcommand::All | Subcommand::UnusedExports);
    let want_targets = matches!(sub, Subcommand::All | Subcommand::Targets);
    let want_hotspots = matches!(sub, Subcommand::All | Subcommand::Hotspots | Subcommand::Targets);

    let mut raw_hotspots: Vec<Hotspot> = Vec::new();
    if want_hotspots {
        match hotspots::analyze_hotspots(repo_root, graph.as_ref(), config.min_commits) {
            Ok(h) => raw_hotspots = h,
            Err(_) => eprintln!("{}", "  git hotspot analysis skipped (not a git repo?)".dimmed()),
        }
    }

    let max_cyc = config.max_complexity as u16;
    let max_cog = config.max_cognitive as u16;

    let (mut complexity, mut large) = (Vec::new(), Vec::new());
    if let (true, Some(graph)) = (want_complexity, &graph) {
        complexity = analyze::complexity::collect(&graph.files, repo_root, max_cyc, max_cog);
        large = analyze::large_functions::collect(&graph.files, repo_root);
    }

    // cycles + unused_exports are needed by refactoring targets even when their own sections aren't shown
    let mut cycles = Vec::new();
    if let (true, Some(graph)) = (want_circular || want_targets, &graph) {
        cycles = analyze::circular::collect(graph, repo_root);
    }
    let mut unused_exports = Vec::new();
    if let (true, Some(graph)) = (want_unused_exports || want_targets, &graph) {
        unused_exports = analyze::unused_exports::collect(&graph.files, repo_root);
    }

    let mut unused_files = Vec::new();
    if let (true, Some(graph)) = (want_unused_files, &graph) {
        unused_files = analyze::unused_files::collect(graph, repo_root);
    }

    let mut targets = Vec::new();
    if let (true, Some(graph)) = (want_targets, &graph) {
        targets = analyze::refactoring_targets::collect(
            graph,
            &raw_hotspots,
            &cycles,
            &unused_exports,
            repo_root,
        );
    }

    let dupes = if matches!(sub, Subcommand::All | Subcommand::Duplicates) {
        Some(analyze::duplicates::analyze(root, repo_root, config.min_tokens, config.min_lines))
    } else {
        None
    };

    let mut deps = if sub == Subcommand::Deps { Some(analyze::deps::analyze(root)) } else { None };
    if !filter.deps_shown() {
        if let Some(Ok(report)) = &mut deps {
            report.unused.clear();
            report.undeclared.clear();
        }
    }

    let hotspots = if filter.hotspots_omitted() {
        Vec::new()
    } else {
        raw_hotspots.into_iter().filter(|h| !filter.skipped(&h.file)).collect()
    };

    complexity.retain(|v| {
        let (s, e) = fn_span(v.line, v.line_count);
        filter.span_shown(&v.path, s, e)
    });
    large.retain(|f| {
        let (s, e) = fn_span(f.line, f.line_count);
        filter.span_shown(&f.path, s, e)
    });
    unused_exports.retain(|u| filter.span_shown(&u.path, u.line, u.line));
    unused_files.retain(|f| filter.added_shown(&f.path));
    cycles.retain(|c| filter.multi_shown(&c.files));
    targets.retain(|t| filter.file_shown(&t.path));

    // only keep cycles/exports if their sections are actually requested (targets borrowed them)
    if !want_circular {
        cycles.clear();
    }
    if !want_unused_exports {
        unused_exports.clear();
    }

    let (clone_groups, clone_families, has_dupes) = match dupes {
        Some(report) => {
            let groups: Vec<CloneGroup> = report
                .groups
                .into_iter()
                .filter(|g| {
                    let members: Vec<(String, u32, u32)> = g
                        .instances
                        .iter()
                        .map(|i| (i.file.clone(), i.start_line, i.end_line))
                        .collect();
                    filter.group_shown(&members)
                })
                .collect();
            let families: Vec<CloneFamily> =
                report.families.into_iter().filter(|f| filter.multi_shown(&f.files)).collect();
            (groups, families, true)
        }
        None => (Vec::new(), Vec::new(), false),
    };

    JavaResult {
        findings,
        hotspots,
        complexity,
        large,
        cycles,
        unused_files,
        unused_exports,
        targets,
        clone_groups,
        clone_families,
        has_dupes,
        deps,
    }
}

fn legend(config: &Config, id: &str) {
    if config.legend {
        if let Some(text) = explain::legend_for(id) {
            report::print_legend(text);
        }
    }
}

fn render_results(config: &Config, filter: &ReportFilter, result: &JavaResult) {
    let sub = config.sub;

    if matches!(sub, Subcommand::All | Subcommand::Hotspots) && !filter.hotspots_omitted() {
        let shown = result.hotspots.len().min(config.top);
        report::print_section(
            "hotspots",
            &format!("top {} of {} files", shown, result.hotspots.len()),
        );
        legend(config, "hotspots");
        report::print_hotspots(&result.hotspots, config.top, &config.cwd);
    }

    if matches!(sub, Subcommand::All | Subcommand::Complexity) {
        report::print_section_count("high complexity functions", result.complexity.len());
        legend(config, "complexity");
        report::print_complexity(
            &result.complexity,
            config.top,
            config.max_complexity as u16,
            config.max_cognitive as u16,
        );

        report::print_section_count("large functions", result.large.len());
        legend(config, "large-functions");
        report::print_large_functions(&result.large, config.top);
    }

    if matches!(sub, Subcommand::All | Subcommand::Circular) {
        report::print_section_count("circular dependencies", result.cycles.len());
        legend(config, "circular");
        report::print_circular(&result.cycles, config.top);
    }

    if matches!(sub, Subcommand::All | Subcommand::UnusedFiles) {
        report::print_section_count("unused files", result.unused_files.len());
        legend(config, "unused-files");
        report::print_unused_files(&result.unused_files, config.top);
    }

    if matches!(sub, Subcommand::All | Subcommand::UnusedExports) {
        report::print_section_count("unused exports", result.unused_exports.len());
        legend(config, "unused-exports");
        report::print_unused_exports(&result.unused_exports, config.top);
    }

    if matches!(sub, Subcommand::All | Subcommand::Targets) {
        report::print_section_count("refactoring targets", result.targets.len());
        legend(config, "targets");
        report::print_targets(&result.targets, config.top);
    }

    if result.has_dupes {
        report::print_section_count("duplicates", result.clone_groups.len());
        legend(config, "duplicates");
        report::print_clone_groups(&result.clone_groups, config.top);
        report::print_section_count("clone families", result.clone_families.len());
        legend(config, "clone-families");
        report::print_clone_families(&result.clone_families, config.top);
    }

    if let Some(deps) = &result.deps {
        match deps {
            Ok(report) => {
                report::print_section_count("unused dependencies", report.unused.len());
                legend(config, "deps");
                report::print_dependencies(&report.unused);
                report::print_section_count("undeclared dependencies", report.undeclared.len());
                legend(config, "deps");
                report::print_dependencies(&report.undeclared);
            }
            Err(_) => {
                println!("{}", "    dependency analysis skipped (mvn not found?)".dimmed());
                println!();
            }
        }
    }

    if matches!(sub, Subcommand::Bugs | Subcommand::DeadCode) {
        println!("{}", "  bugs / dead-code — porting to Rust in progress (PLAN.md)".dimmed());
        println!();
    }
}
