import ora from 'ora';
import { isMavenAvailable, compile } from './maven.js';
import { runPmd } from './pmd.js';
import { runSpotbugs } from './spotbugs.js';
import { analyzeHotspots } from './hotspots.js';
import type { Finding, Hotspot, Config, Subcommand } from '../types.js';

export interface JavaResult {
  findings: Finding[];
  hotspots: Hotspot[];
}

function getRunFlags(sub: Subcommand, noCompile: boolean) {
  return {
    compile:  (sub === 'all' || sub === 'bugs') && !noCompile,
    pmd:      sub !== 'hotspots',
    spotbugs: sub === 'all' || sub === 'bugs',
    hotspots: sub === 'all' || sub === 'hotspots',
  };
}

function runStep<T>(label: string, fn: () => T, successFn?: (r: T) => string): T {
  const s = ora({ text: label, prefixText: ' ' }).start();
  try {
    const r = fn();
    s.succeed(successFn ? successFn(r) : label);
    return r;
  } catch (e) {
    s.fail(`${label} failed`);
    throw e;
  }
}

export async function runJava(cwd: string, config: Config): Promise<JavaResult> {
  if (!isMavenAvailable()) {
    console.error('  error: mvn not found — install Maven to analyze Java projects');
    process.exit(2);
  }

  const flags = getRunFlags(config.sub, config.noCompile);
  const findings: Finding[] = [];
  let hotspots: Hotspot[] = [];

  if (flags.compile)  runStep('compiling…',       () => compile(cwd),               () => 'compiled');
  if (flags.pmd)      findings.push(...runStep('running PMD…',      () => runPmd(cwd, config.sub),  r => `PMD — ${r.length} violations`));
  if (flags.spotbugs) findings.push(...runStep('running SpotBugs…', () => runSpotbugs(cwd),         r => `SpotBugs — ${r.length} findings`));

  if (flags.hotspots) {
    const s = ora({ text: 'analyzing git history…', prefixText: ' ' }).start();
    try {
      hotspots = analyzeHotspots(cwd);
      s.succeed(`git hotspots — ${hotspots.length} files ranked`);
    } catch {
      s.warn('git hotspot analysis skipped (not a git repo?)');
    }
  }

  return { findings, hotspots };
}
