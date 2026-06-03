# reap

`reap` is a thin wrapper around [fallow](https://github.com/fallow-rs/fallow) that extends its analysis to Java projects. If your team runs both Java and TypeScript codebases and wants a single tool across all pipelines, `reap` gives you a consistent interface: same commands, same exit codes, same mental model.

**For TypeScript projects, `reap` does nothing on its own — it calls `fallow` directly and gets out of the way.** If you only work with TypeScript, just use `fallow`. `reap` is for teams that also ship Java and don't want to maintain two different analysis setups.

---

## How it works

- **TypeScript projects** — `reap` detects `package.json` / `tsconfig.json` and delegates the call to `fallow` with all arguments forwarded as-is. `fallow` must be installed globally.
- **Java projects** — `reap` detects `pom.xml` and runs its own analysis stack: PMD for static analysis, SpotBugs for bug patterns and security, and a port of fallow's git churn algorithm for hotspot detection.
- **Mixed projects** — runs both.

---

## Requirements

- Node.js 18+
- `fallow` installed globally (`npm install -g fallow`) — required for TypeScript analysis
- Maven (`mvn`) in PATH — required for Java analysis

---

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

For TypeScript projects, subcommands and arguments are forwarded to `fallow` verbatim.

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

This is a direct port of fallow's algorithm, originally described in Adam Tornhill's *Your Code as a Crime Scene*.

---

## Acknowledgements

`reap` wouldn't exist without [fallow](https://github.com/fallow-rs/fallow). The hotspot algorithm, the CLI design, and the overall philosophy are all fallow's — this project just brings the same ideas to the Java ecosystem and wraps them under a common interface. If you work with TypeScript, go use fallow directly; it's significantly more capable there.

---

## License

MIT
