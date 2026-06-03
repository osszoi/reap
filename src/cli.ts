import { Command } from 'commander';
import { DEFAULTS, parseFailOn } from './config.js';
import { detectProject } from './detect.js';
import { runJava } from './java/index.js';
import { runFallow } from './ts/index.js';
import { printHeader, printSection, printFindings, printHotspots, printSummary } from './output.js';
import { checkThresholds } from './threshold.js';
import type { Config, Subcommand, Finding, Hotspot } from './types.js';

const pkg = { version: '0.1.0' };

function addCommonOptions(cmd: Command): Command {
  return cmd
    .option('--fail-on <targets>', 'comma-separated: nullpointers,complexity,dead-code,bugs,hotspots,all', 'nullpointers')
    .option('--max-complexity <n>', 'cyclomatic complexity threshold', String(DEFAULTS.maxComplexity))
    .option('--max-hotspot-score <n>', 'hotspot score threshold (default: no limit)')
    .option('--skip-compile', 'skip Maven compile step')
    .option('--verbose', 'show medium/low findings too')
    .option('--top <n>', 'how many hotspots to show', String(DEFAULTS.top));
}

// fallow-ignore-next-line complexity
function buildConfig(sub: Subcommand, opts: Record<string, string | boolean | undefined>): Config {
  return {
    sub,
    cwd: process.cwd(),
    failOn: parseFailOn(String(opts['failOn'] ?? 'nullpointers')),
    maxComplexity: parseInt(String(opts['maxComplexity'] ?? DEFAULTS.maxComplexity), 10),
    maxHotspotScore: opts['maxHotspotScore'] ? parseFloat(String(opts['maxHotspotScore'])) : Infinity,
    noCompile: Boolean(opts['skipCompile']),
    verbose: Boolean(opts['verbose']),
    top: parseInt(String(opts['top'] ?? DEFAULTS.top), 10),
  };
}

function hiddenHint(total: number, visible: number): string {
  const hidden = total - visible;
  return hidden > 0 ? `  — ${hidden} medium/low hidden, use --verbose` : '';
}

function pmdSectionLabel(sub: Subcommand): string {
  if (sub === 'complexity') return 'complexity  (PMD)';
  if (sub === 'dead-code')  return 'dead code  (PMD)';
  return 'pmd';
}

function showsHotspots(sub: Subcommand) { return sub === 'all' || sub === 'hotspots'; }
function showsBugs(sub: Subcommand)     { return sub === 'all' || sub === 'bugs'; }
function showsPmd(sub: Subcommand)      { return sub === 'all' || sub === 'dead-code' || sub === 'complexity'; }

function renderResults(config: Config, findings: Finding[], hotspots: Hotspot[]) {
  const { sub, verbose, top } = config;
  const visible = verbose
    ? findings
    : findings.filter(f => f.severity === 'critical' || f.severity === 'high');

  if (showsHotspots(sub)) {
    printSection('hotspots', `top ${Math.min(hotspots.length, top)} of ${hotspots.length} files`);
    printHotspots(hotspots, top);
  }

  if (sub === 'hotspots') return;

  const sbAll     = findings.filter(f => f.source === 'spotbugs');
  const pmdAll    = findings.filter(f => f.source === 'pmd');
  const sbVisible  = visible.filter(f => f.source === 'spotbugs');
  const pmdVisible = visible.filter(f => f.source === 'pmd');

  if (showsBugs(sub)) {
    printSection(`bugs  (SpotBugs)${hiddenHint(sbAll.length, sbVisible.length)}`, sbVisible.length);
    printFindings(sbVisible);
  }

  if (showsPmd(sub)) {
    printSection(`${pmdSectionLabel(sub)}${hiddenHint(pmdAll.length, pmdVisible.length)}`, pmdVisible.length);
    printFindings(pmdVisible);
  }
}

async function runAnalysis(sub: Subcommand, opts: Record<string, string | boolean | undefined>) {
  const config = buildConfig(sub, opts);
  const project = detectProject(config.cwd);

  if (project.lang === 'unknown') {
    console.error('  error: no pom.xml or package.json found — are you in a project directory?');
    process.exit(2);
  }

  if (project.lang === 'ts') {
    runFallow(sub === 'all' ? [] : [sub, ...process.argv.slice(3)]);
  }

  printHeader(pkg.version, project.name, project.lang === 'both' ? 'Java + TypeScript' : 'Java');
  console.log();

  const { findings, hotspots } = await runJava(project.root, config);
  console.log();

  renderResults(config, findings, hotspots);

  const { passed, reasons } = checkThresholds(config, findings, hotspots);
  printSummary(passed, reasons);
  if (!passed) process.exit(1);
}

const program = new Command()
  .name('reap')
  .description('Code health scanner — Java and TypeScript')
  .version(pkg.version);

addCommonOptions(program).action((opts) => runAnalysis('all', opts));

addCommonOptions(program.command('hotspots').description('git churn × complexity ranking'))
  .action((opts) => runAnalysis('hotspots', opts));

addCommonOptions(program.command('dead-code').description('unused variables, imports, methods'))
  .action((opts) => runAnalysis('dead-code', opts));

addCommonOptions(program.command('complexity').description('cyclomatic and cognitive complexity'))
  .action((opts) => runAnalysis('complexity', opts));

addCommonOptions(program.command('bugs').description('SpotBugs bug patterns and security findings'))
  .action((opts) => runAnalysis('bugs', opts));

program.parseAsync(process.argv).catch(e => {
  console.error('  error:', e.message);
  process.exit(2);
});
