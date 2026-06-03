use owo_colors::OwoColorize;

pub struct Topic {
    pub id: &'static str,
    pub aliases: &'static [&'static str],
    pub name: &'static str,
    pub legend: &'static str,
    pub full: &'static str,
    pub how_to_fix: &'static str,
}

pub static TOPICS: &[Topic] = &[
    Topic {
        id: "complexity",
        aliases: &[
            "cyclomatic",
            "cognitive",
            "high-complexity",
            "high-complexity-functions",
            "complexity-functions",
        ],
        name: "High Complexity Functions",
        legend: "cyclomatic = independent paths (1 + each if/for/while/case/catch/&&/|| /?:); >20 flagged, ≥30 high, ≥50 critical — e.g. 42 ≈ 42 paths, very hard to test. cognitive = reading difficulty (penalizes nesting); >15 flagged, ≥25 high, ≥40 critical.",
        full: "Two complementary measures of how hard a function is to understand and test.\n\
               \n\
               Cyclomatic complexity (McCabe) counts the independent paths through a function: \
               start at 1 and add one for every branching point — if, for, while, case, catch, \
               ternary, and each && or || in a condition. A value of 42 means roughly 42 distinct \
               paths, so you'd need ~42 test cases for full path coverage. >20 is flagged, ≥30 is \
               high, ≥50 is critical.\n\
               \n\
               Cognitive complexity (SonarSource) measures how hard the code is to READ rather than \
               to test. It penalizes nested control flow more heavily (a branch three levels deep \
               costs more than one at the top) and treats else-if/logical runs more naturally than \
               cyclomatic does. >15 is flagged, ≥25 is high, ≥40 is critical.",
        how_to_fix: "Extract nested blocks into well-named helper methods; replace deep if/else \
                     ladders with early returns or guard clauses; collapse boolean chains into named \
                     predicates; consider polymorphism or a lookup table instead of large switch \
                     statements.",
    },
    Topic {
        id: "large-functions",
        aliases: &["large", "large-functions", "long-functions", "long"],
        name: "Large Functions",
        legend: "functions longer than 60 lines (very high risk); ranked by length.",
        full: "Functions whose body exceeds 60 lines. Long functions tend to do several things at \
               once, are harder to name, harder to test in isolation, and accumulate complexity over \
               time. Length is a blunt but reliable smell that usually correlates with mixed \
               responsibilities.",
        how_to_fix: "Split the function along its natural seams — each distinct step or responsibility \
                     becomes its own method. Look for comment-delimited sections; those are almost \
                     always extractable units.",
    },
    Topic {
        id: "hotspots",
        aliases: &["hotspot", "churn"],
        name: "Hotspots",
        legend: "score 0–100 = recency-weighted churn × complexity-density, normalized to the worst file. density = total cyclomatic ÷ lines. fan-in = files importing this. ▲ accelerating · ▼ cooling · ─ stable.",
        full: "Hotspots rank files by where churn and complexity collide — the code most likely to \
               cause maintenance pain. The score (0–100) multiplies recency-weighted git churn by \
               complexity density and normalizes against the worst file in the project.\n\
               \n\
               Churn is weighted with a 90-day half-life, so recent edits count more than old ones. \
               Density is total cyclomatic complexity divided by lines. Fan-in is how many other \
               files import this one (high fan-in means a change here ripples widely). The trend \
               arrow compares recent vs older commit rates: ▲ accelerating (>1.5×), ▼ cooling \
               (<0.67×), ─ stable.",
        how_to_fix: "Prioritize hotspots for refactoring, extra tests, and review attention. A file \
                     that's both complex and frequently changed is your highest-leverage place to \
                     reduce risk.",
    },
    Topic {
        id: "circular",
        aliases: &["circular-dependencies", "cycles", "circular-deps", "circular-dependency"],
        name: "Circular Dependencies",
        legend: "import cycles between classes — each entry is a group that (transitively) imports itself; (cross-package) = spans Maven modules.",
        full: "Groups of classes that import each other in a cycle (directly or transitively). \
               Cycles make code hard to understand and impossible to compile, test, or reason about \
               in isolation — you can't pull one class out without dragging the rest along. \
               Cross-package cycles (spanning Maven modules) are worse: they break modular \
               boundaries you presumably set up on purpose.",
        how_to_fix: "Break the cycle by introducing an interface one side depends on, moving the \
                     shared type to a third location, or inverting a dependency (dependency \
                     inversion). Cross-package cycles usually signal a misplaced class — move it to \
                     the module that truly owns it.",
    },
    Topic {
        id: "unused-files",
        aliases: &["dead-files", "unreachable-files"],
        name: "Unused Files",
        legend: "files unreachable from any entry point (main / tests / Spring beans / SPI services).",
        full: "Files that cannot be reached from any entry point by following imports. Entry points \
               include main methods, test classes, Spring-managed beans, and SPI service providers. \
               If nothing reachable references a file, it's likely dead code that can be deleted.",
        how_to_fix: "Confirm the file truly isn't loaded reflectively or by a framework, then delete \
                     it. If it IS an entry point reap doesn't recognize (e.g. a custom framework \
                     hook), that's a false positive — the reachability roots may need extending.",
    },
    Topic {
        id: "unused-exports",
        aliases: &["dead-exports", "exports", "unused-export"],
        name: "Unused Exports",
        legend: "public/protected methods with no caller in any other file (heuristic: name-based; skips @Override, main, tests).",
        full: "Public or protected methods that no other file appears to call. This is a name-based \
               heuristic: it scans every file for the method name being referenced. It deliberately \
               skips @Override methods, main, and test methods. Because it's name-based, a method \
               called only via reflection or by an external consumer of your library will show up \
               here as a false positive.",
        how_to_fix: "If the method is genuinely internal and unused, delete it or reduce its \
                     visibility to private/package-private. If it's part of a public library API \
                     consumed externally, it's expected to appear here — reap analyzes only this \
                     codebase.",
    },
    Topic {
        id: "deps",
        aliases: &[
            "dependencies",
            "unused-dependencies",
            "undeclared-dependencies",
            "unused-deps",
            "maven",
        ],
        name: "Dependencies",
        legend: "unused = declared in pom.xml but never imported. undeclared = imported but only available transitively.",
        full: "Maven dependency hygiene, sourced from `mvn dependency:analyze`.\n\
               \n\
               Unused dependencies are declared in your pom.xml but nothing in your code imports \
               them — dead weight that slows builds and bloats the classpath. Undeclared \
               dependencies are the dangerous ones: your code imports them but they're only present \
               transitively (pulled in by something else). A transitive bump or removal upstream can \
               break your build with no warning.",
        how_to_fix: "Remove unused dependencies from pom.xml. For undeclared dependencies, add an \
                     explicit <dependency> entry so you control the version directly instead of \
                     relying on a transitive accident.",
    },
    Topic {
        id: "duplicates",
        aliases: &["dupes", "duplicate-code", "clones", "clone"],
        name: "Duplicates",
        legend: "identical token sequences (≥50 tokens, ≥5 lines). \"N instances\" = number of copies; line range is per copy.",
        full: "Blocks of code that are token-for-token identical across the project, found via a \
               suffix-array scan (default thresholds: ≥50 tokens and ≥5 lines). Each group lists \
               every copy and its line range. Duplication means a bug fixed in one copy stays broken \
               in the others, and behavior drifts as copies are edited independently.",
        how_to_fix: "Extract the duplicated block into a single shared method or class and call it \
                     from each site. If the copies have diverged slightly, reconcile the differences \
                     first, then unify.",
    },
    Topic {
        id: "clone-families",
        aliases: &["families", "clone-family", "clone-families"],
        name: "Clone Families",
        legend: "clone groups spanning the same set of files — candidates to extract into shared code.",
        full: "A clone family groups together duplicate blocks that recur across the same set of \
               files. Where a single duplicate group is one repeated block, a family reveals that \
               two or more files share multiple duplicated blocks — a strong signal they should \
               share a common abstraction (a base class, a utility, a template).",
        how_to_fix: "Look at the files in the family as a unit: they likely want a shared base class, \
                     helper, or strategy. Extracting the common structure once removes several \
                     duplicate groups at a time.",
    },
    Topic {
        id: "targets",
        aliases: &["refactoring-targets", "refactor", "refactoring", "target"],
        name: "Refactoring Targets",
        legend: "efficiency = priority ÷ effort (quick-win ROI, higher = better). pri = absolute priority 0–100. category = the dominant signal that flagged it.",
        full: "A prioritized, ROI-ranked list of what to refactor first. It fuses every other signal \
               — hotspot score, complexity, cycles, duplication, unused exports — into one ranking.\n\
               \n\
               Priority (0–100) is the absolute importance of fixing a file. Effort estimates how \
               much work it would take. Efficiency = priority ÷ effort, so high-efficiency targets \
               are quick wins: a lot of risk removed for relatively little work. Category names the \
               dominant problem that flagged the file.",
        how_to_fix: "Work top-down by efficiency for the best return on effort. The category tells you \
                     what kind of fix is needed; cross-reference the matching section (complexity, \
                     duplicates, etc.) for the specifics.",
    },
];

pub fn normalize(query: &str) -> String {
    query.trim().to_lowercase().replace([' ', '_'], "-")
}

pub fn find_topic(query: &str) -> Option<&'static Topic> {
    let key = normalize(query);
    TOPICS
        .iter()
        .find(|t| t.id == key || t.aliases.contains(&key.as_str()))
}

pub fn legend_for(id: &str) -> Option<&'static str> {
    TOPICS.iter().find(|t| t.id == id).map(|t| t.legend)
}

pub fn run(query: &str) {
    if query.trim().is_empty() {
        print_topic_list();
        return;
    }

    match find_topic(query) {
        Some(topic) => print_topic(topic),
        None => {
            println!();
            println!("{} unknown topic: {}", "  ✖".red().bold(), query.trim().yellow());
            print_topic_list();
        }
    }
}

fn print_topic(topic: &Topic) {
    println!();
    println!("  {}", topic.name.bold().cyan());
    println!("{}", format!("  {}", "─".repeat(56)).dimmed());
    println!();
    for line in topic.full.lines() {
        println!("  {}", line.trim_start());
    }
    println!();
    println!("  {}", "metric".bold());
    println!("  {}", topic.legend.dimmed());
    println!();
    println!("  {}", "how to fix".bold());
    println!("  {}", topic.how_to_fix);
    println!();
}

fn print_topic_list() {
    println!();
    println!("  {}", "available topics".bold());
    println!("{}", format!("  {}", "─".repeat(56)).dimmed());
    for t in TOPICS {
        println!("  {}  {}", format!("{:<18}", t.id).cyan(), t.name.dimmed());
    }
    println!();
    println!("{}", "  usage: reap explain <topic>   e.g. reap explain cyclomatic".dimmed());
    println!();
}
