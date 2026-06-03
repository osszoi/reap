import { Command } from 'commander';
import { DEFAULTS, parseFailOn } from './config.js';
import { detectProject } from './detect.js';
import { runJava } from './java/index.js';
import { runFallow } from './ts/index.js';
import { printHeader, printSection, printFindings, printHotspots, printSummary } from './output.js';
import { checkThresholds } from './threshold.js';
import type { Config, Subcommand } from './types.js';

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

  const langLabel = project.lang === 'both' ? 'Java + TypeScript' : 'Java';
  printHeader(pkg.version, project.name, langLabel);

  console.log();
  const { findings, hotspots } = await runJava(project.root, config);
  console.log();

  const showMedLow = config.verbose;
  const visibleFindings = showMedLow
    ? findings
    : findings.filter(f => f.severity === 'critical' || f.severity === 'high');

  if (sub === 'all' || sub === 'hotspots') {
    const shown = Math.min(hotspots.length, config.top);
    printSection('hotspots', `top ${shown} of ${hotspots.length} files`);
    printHotspots(hotspots, config.top);
  }

  if (sub !== 'hotspots') {
    const sbFindings = visibleFindings.filter(f => f.source === 'spotbugs');
    const pmdFindings = visibleFindings.filter(f => f.source === 'pmd');

    if (sub === 'all' || sub === 'bugs') {
      const total = findings.filter(f => f.source === 'spotbugs').length;
      const hidden = total - sbFindings.length;
      const label = `bugs  (SpotBugs)${hidden > 0 ? `  — ${hidden} medium/low hidden, use --verbose` : ''}`;
      printSection(label, sbFindings.length);
      printFindings(sbFindings);
    }

    if (sub === 'all' || sub === 'dead-code' || sub === 'complexity') {
      const total = findings.filter(f => f.source === 'pmd').length;
      const hidden = total - pmdFindings.length;
      const base = sub === 'complexity' ? 'complexity  (PMD)' : sub === 'dead-code' ? 'dead code  (PMD)' : 'pmd';
      const label = `${base}${hidden > 0 ? `  — ${hidden} medium/low hidden, use --verbose` : ''}`;
      printSection(label, pmdFindings.length);
      printFindings(pmdFindings);
    }
  }

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
