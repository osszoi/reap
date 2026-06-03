# reap

Code health scanner for Java and TypeScript. Combines static analysis (PMD, SpotBugs) with git churn history to surface what actually matters — not just what the linter complains about.

Named after its companion tool [fallow](https://github.com/fallow-rs/fallow). For TypeScript projects, `reap` delegates directly to `fallow`. For Java, it runs its own analysis stack.

---

## Requirements

- Node.js 18+
- Java projects: Maven (`mvn`) in PATH
- TypeScript projects: `fallow` installed globally

## Install

```sh
npx reap
```

Or globally:

```sh
npm install -g reap
```

---

## Usage

```sh
reap                   # full scan: hotspots + SpotBugs + PMD
reap hotspots          # git churn analysis only (no Maven required)
reap bugs              # SpotBugs findings
reap dead-code         # unused variables, imports, private methods
reap complexity        # cyclomatic and cognitive complexity
```

For TypeScript projects, any subcommand and its arguments are forwarded to `fallow` as-is.

---

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--fail-on <targets>` | `nullpointers` | Comma-separated fail conditions (see below) |
| `--max-complexity <n>` | `20` | Cyclomatic complexity threshold |
| `--max-hotspot-score <n>` | none | Fail if top hotspot exceeds this score |
| `--skip-compile` | — | Skip `mvn compile` (use when already built) |
| `--verbose` | — | Show medium and low severity findings |
| `--top <n>` | `20` | Number of hotspots to display |

### `--fail-on` targets

| Value | Fails when |
|-------|-----------|
| `nullpointers` | Any SpotBugs `NP_*` finding |
| `bugs` | Any SpotBugs finding of high severity or above |
| `complexity` | Any method exceeds the complexity threshold |
| `dead-code` | Any unused variable, import, or private method |
| `hotspots` | Any file exceeds `--max-hotspot-score` |
| `all` | Any critical or high finding across all sources |

Multiple targets: `--fail-on=nullpointers,complexity`

---

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | All thresholds passed |
| `1` | One or more `--fail-on` conditions triggered |
| `2` | Tool error (Maven not found, not a project directory, etc.) |

---

## CI usage

After your build step:

```yaml
- run: npx reap --skip-compile --fail-on=nullpointers,complexity
```

To enforce strict quality gates:

```yaml
- run: npx reap --skip-compile --fail-on=all --max-complexity=15
```

---

## How hotspots work

Hotspot score = recency-weighted commit frequency × file size factor. Commits are decayed with a 90-day half-life, so files that churned last month rank higher than files that churned years ago and stabilized. A large file that keeps changing is more dangerous than a small one.

This is the same approach used by [fallow](https://github.com/fallow-rs/fallow) and originally described in Adam Tornhill's *Your Code as a Crime Scene*.

---

## License

MIT
