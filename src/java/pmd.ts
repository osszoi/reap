import { execSync } from 'child_process';
import { XMLParser } from 'fast-xml-parser';
import fs from 'fs';
import type { Finding, Severity, Subcommand } from '../types.js';
import { findReports, toArray } from './utils.js';

const PMD_PLUGIN = 'org.apache.maven.plugins:maven-pmd-plugin:3.22.0';

const RULESETS_BY_SUB: Record<string, string> = {
  'dead-code': 'category/java/bestpractices.xml,category/java/codestyle.xml',
  complexity:  'category/java/design.xml',
  bugs:        'category/java/errorprone.xml,category/java/performance.xml,category/java/bestpractices.xml',
  all:         'category/java/bestpractices.xml,category/java/design.xml,category/java/errorprone.xml,category/java/performance.xml,category/java/codestyle.xml',
};

export function runPmd(cwd: string, sub: Subcommand): Finding[] {
  const rulesets = RULESETS_BY_SUB[sub] ?? RULESETS_BY_SUB['all'];
  try {
    execSync(
      `mvn ${PMD_PLUGIN}:pmd -Dpmd.rulesets="${rulesets}" --no-transfer-progress -q`,
      { cwd, maxBuffer: 50 * 1024 * 1024, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }
    );
  } catch { /* non-zero exit expected when violations found — reports still written */ }
  return collectPmdReports(cwd);
}

function collectPmdReports(cwd: string): Finding[] {
  const xmlFiles = findReports(cwd, 'pmd.xml');
  const parser = new XMLParser({ removeNSPrefix: true, ignoreAttributes: false, attributeNamePrefix: '@_' });
  const findings: Finding[] = [];
  for (const xmlPath of xmlFiles) {
    const doc = parser.parse(fs.readFileSync(xmlPath, 'utf8'));
    if (!doc?.pmd) continue;
    for (const fileEl of toArray<Record<string, unknown>>(doc.pmd.file)) {
      findings.push(...parseFileFindings(fileEl));
    }
  }
  return findings;
}

// fallow-ignore-next-line complexity
function parseViolation(v: Record<string, unknown>, file: string): Finding {
  return {
    file,
    line:     String(v['@_beginline'] ?? '?'),
    rule:     String(v['@_rule'] ?? '?'),
    ruleset:  String(v['@_ruleset'] ?? '?'),
    severity: pmdPriority(parseInt(String(v['@_priority'] ?? '5'), 10)),
    message:  (typeof v === 'string' ? v : String(v['#text'] ?? '')).trim(),
    url:      v['@_externalInfoUrl'] as string | undefined,
    source:   'pmd' as const,
  };
}

function parseFileFindings(fileEl: Record<string, unknown>): Finding[] {
  const file = String(fileEl['@_name'] ?? '');
  return toArray<Record<string, unknown>>(fileEl.violation as Record<string, unknown>[])
    .map(v => parseViolation(v, file));
}

// fallow-ignore-next-line complexity
function pmdPriority(p: number): Severity {
  if (p <= 2) return 'critical';
  if (p === 3) return 'high';
  if (p === 4) return 'medium';
  return 'low';
}
