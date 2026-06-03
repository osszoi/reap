import { execSync } from 'child_process';
import { XMLParser } from 'fast-xml-parser';
import fs from 'fs';
import path from 'path';
import type { Finding, Severity, Subcommand } from '../types.js';

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
  } catch {
    // non-zero exit is expected when violations are found — reports are still written
  }

  return collectPmdReports(cwd);
}

function collectPmdReports(cwd: string): Finding[] {
  const xmlFiles = findReports(cwd, 'pmd.xml');
  const parser = new XMLParser({ removeNSPrefix: true, ignoreAttributes: false, attributeNamePrefix: '@_' });
  const findings: Finding[] = [];

  for (const xmlPath of xmlFiles) {
    const content = fs.readFileSync(xmlPath, 'utf8');
    const doc = parser.parse(content);
    const pmd = doc?.pmd;
    if (!pmd) continue;

    const files = Array.isArray(pmd.file) ? pmd.file : pmd.file ? [pmd.file] : [];
    for (const fileEl of files) {
      const fileName: string = fileEl['@_name'] ?? '';
      const violations = Array.isArray(fileEl.violation) ? fileEl.violation : fileEl.violation ? [fileEl.violation] : [];
      for (const v of violations) {
        const priority = parseInt(v['@_priority'] ?? '5', 10);
        findings.push({
          file: fileName,
          line: v['@_beginline'] ?? '?',
          rule: v['@_rule'] ?? '?',
          ruleset: v['@_ruleset'] ?? '?',
          severity: pmdPriority(priority),
          message: (typeof v === 'string' ? v : v['#text'] ?? '').trim(),
          url: v['@_externalInfoUrl'],
          source: 'pmd',
        });
      }
    }
  }

  return findings;
}

function pmdPriority(p: number): Severity {
  if (p <= 2) return 'critical';
  if (p === 3) return 'high';
  if (p === 4) return 'medium';
  return 'low';
}

function findReports(cwd: string, name: string): string[] {
  const results: string[] = [];
  const scan = (dir: string, depth: number) => {
    if (depth > 4) return;
    try {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (entry.isDirectory() && entry.name !== 'node_modules') {
          scan(path.join(dir, entry.name), depth + 1);
        } else if (entry.name === name) {
          results.push(path.join(dir, entry.name));
        }
      }
    } catch { /* ignore */ }
  };
  scan(cwd, 0);
  return results;
}
