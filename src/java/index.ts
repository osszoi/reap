import ora from 'ora';
import { isMavenAvailable, compile } from './maven.js';
import { runPmd } from './pmd.js';
import { runSpotbugs } from './spotbugs.js';
import { analyzeHotspots } from './hotspots.js';
import type { Finding, Hotspot, Config } from '../types.js';

export interface JavaResult {
  findings: Finding[];
  hotspots: Hotspot[];
}

export async function runJava(cwd: string, config: Config): Promise<JavaResult> {
  if (!isMavenAvailable()) {
    console.error('  error: mvn not found — install Maven to analyze Java projects');
    process.exit(2);
  }

  const needsCompile = (config.sub === 'all' || config.sub === 'bugs') && !config.noCompile;
  const needsPmd     = config.sub !== 'hotspots';
  const needsSb      = config.sub === 'all' || config.sub === 'bugs';
  const needsHots    = config.sub === 'all' || config.sub === 'hotspots';

  let findings: Finding[] = [];
  let hotspots: Hotspot[] = [];

  if (needsCompile) {
    const spinner = ora({ text: 'compiling…', prefixText: ' ' }).start();
    try {
      compile(cwd);
      spinner.succeed('compiled');
    } catch (e) {
      spinner.fail('compile failed');
      throw e;
    }
  }

  if (needsPmd) {
    const spinner = ora({ text: 'running PMD…', prefixText: ' ' }).start();
    try {
      const pmdFindings = runPmd(cwd, config.sub);
      findings.push(...pmdFindings);
      spinner.succeed(`PMD — ${pmdFindings.length} violations`);
    } catch (e) {
      spinner.fail('PMD failed');
      throw e;
    }
  }

  if (needsSb) {
    const spinner = ora({ text: 'running SpotBugs…', prefixText: ' ' }).start();
    try {
      const sbFindings = runSpotbugs(cwd);
      findings.push(...sbFindings);
      spinner.succeed(`SpotBugs — ${sbFindings.length} findings`);
    } catch (e) {
      spinner.fail('SpotBugs failed');
      throw e;
    }
  }

  if (needsHots) {
    const spinner = ora({ text: 'analyzing git history…', prefixText: ' ' }).start();
    try {
      hotspots = analyzeHotspots(cwd);
      spinner.succeed(`git hotspots — ${hotspots.length} files ranked`);
    } catch (e) {
      spinner.warn('git hotspot analysis skipped (not a git repo?)');
    }
  }

  return { findings, hotspots };
}
