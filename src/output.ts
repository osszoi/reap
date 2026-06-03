import chalk from 'chalk';
import path from 'path';
import type { Finding, Hotspot } from './types.js';

const SEV: Record<string, (s: string) => string> = {
  critical: (s) => chalk.red.bold(s),
  high:     (s) => chalk.yellow.bold(s),
  medium:   (s) => chalk.yellow(s),
  low:      (s) => chalk.blue(s),
};

const DOT: Record<string, string> = {
  critical: '●',
  high:     '●',
  medium:   '◦',
  low:      '◦',
};

export function printHeader(version: string, name: string, lang: string) {
  console.log();
  process.stdout.write(chalk.bold.cyan('  reap'));
  process.stdout.write(chalk.dim(` v${version}  ·  `));
  process.stdout.write(chalk.white(name));
  process.stdout.write(chalk.dim(`  ·  ${lang}`));
  console.log('\n');
}

export function printSection(title: string, count: number | string) {
  const cnt = typeof count === 'number'
    ? (count === 0 ? chalk.green(String(count)) : chalk.yellow(String(count)))
    : chalk.dim(count);
  console.log(chalk.cyan.bold(`  ⬡ ${title}`) + chalk.dim('  ') + cnt);
  console.log(chalk.dim('  ' + '─'.repeat(56)));
}

export function printHotspots(hotspots: Hotspot[], top: number) {
  if (hotspots.length === 0) {
    console.log(chalk.dim('    no hotspots found'));
    console.log();
    return;
  }

  hotspots.slice(0, top).forEach((h, i) => {
    const scoreStr = h.score.toFixed(1).padStart(5);
    const scoreColored =
      h.score > 15 ? chalk.red.bold(scoreStr) :
      h.score > 8  ? chalk.yellow(scoreStr) :
                     chalk.blue(scoreStr);

    const rank = chalk.dim(`  ${String(i + 1).padStart(2)}.`);
    const fname = path.basename(h.file).padEnd(44);
    const meta = chalk.dim(`${h.totalCommits} commits  ${h.loc} LOC`);
    console.log(`${rank}  ${scoreColored}  ${chalk.white(fname)}  ${meta}`);
  });
  console.log();
}

export function printFindings(findings: Finding[]) {
  if (findings.length === 0) {
    console.log(chalk.dim('    no findings'));
    console.log();
    return;
  }

  for (const f of findings) {
    const icon = DOT[f.severity] ?? '◦';
    const sevLabel = (f.severity).padEnd(8);
    const sev = SEV[f.severity]?.(icon + ' ' + sevLabel) ?? sevLabel;
    const rule = chalk.dim(f.rule);
    const loc  = chalk.dim(`    ${shorten(f.file)}:${f.line}`);
    const msg  = '  ' + f.message;

    console.log(`  ${sev}  ${rule}`);
    console.log(loc + msg);
    console.log();
  }
}

export function printSummary(passed: boolean, reasons: string[]) {
  console.log(chalk.dim('  ' + '─'.repeat(56)));
  if (passed) {
    console.log(chalk.green.bold('  ✔  passed — no violations above threshold'));
  } else {
    for (const r of reasons) {
      console.log(chalk.red.bold(`  ✖  ${r}`));
    }
  }
  console.log();
}

function shorten(p: string): string {
  return p
    .replace(/.*\/src\/(?:main|test)\/java\/com\/dawere\/backend\//, '~/')
    .replace(/.*\/src\/main\/java\/com\/dawere\//, 'sdk/')
    .replace(process.cwd() + '/', '');
}
