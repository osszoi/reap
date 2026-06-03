import { execSync } from 'child_process';
import { XMLParser } from 'fast-xml-parser';
import fs from 'fs';
import path from 'path';
import type { Finding, Severity } from '../types.js';

const SB_PLUGIN = 'com.github.spotbugs:spotbugs-maven-plugin:4.8.4.0';

export function runSpotbugs(cwd: string): Finding[] {
  const env = {
    ...process.env,
    MAVEN_OPTS: `${process.env['MAVEN_OPTS'] ?? ''} -Djdk.xml.entityExpansionLimit=0`.trim(),
  };

  try {
    execSync(
      `mvn ${SB_PLUGIN}:spotbugs -Dspotbugs.effort=Max -Dspotbugs.threshold=Low --no-transfer-progress -q`,
      { cwd, env, maxBuffer: 50 * 1024 * 1024, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }
    );
  } catch {
    // spotbugs exits non-zero when bugs found — reports are still written
  }

  return collectSpotbugsReports(cwd);
}

function collectSpotbugsReports(cwd: string): Finding[] {
  const xmlFiles = findReports(cwd, 'spotbugsXml.xml');
  const parser = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: '@_', processEntities: false });
  const findings: Finding[] = [];

  for (const xmlPath of xmlFiles) {
    const content = fs.readFileSync(xmlPath, 'utf8');
    const doc = parser.parse(content);
    const bugs = doc?.BugCollection?.BugInstance;
    if (!bugs) continue;

    const list = Array.isArray(bugs) ? bugs : [bugs];
    for (const bug of list) {
      const bugType: string = bug['@_type'] ?? '';
      const rank = parseInt(bug['@_rank'] ?? '20', 10);
      const category: string = bug['@_category'] ?? '';

      // filter low-signal noise: MALICIOUS_CODE with rank > 14 is mostly "expose defensive copies" warnings
      if (category === 'MALICIOUS_CODE' && rank > 14 && !bugType.startsWith('NP_')) continue;

      const srcLines = Array.isArray(bug.SourceLine) ? bug.SourceLine : bug.SourceLine ? [bug.SourceLine] : [];
      const primarySrc = srcLines.find((s: Record<string, string>) => s['@_primary'] === 'true') ?? srcLines[0];
      const file = primarySrc?.['@_sourcepath'] ?? '?';
      const line = primarySrc?.['@_start'] ?? '?';

      const msg = bug.LongMessage ?? bug['@_type'] ?? '?';

      findings.push({
        file,
        line,
        rule: bugType,
        ruleset: category,
        severity: sbSeverity(bugType, rank),
        message: typeof msg === 'string' ? msg.trim() : String(msg),
        source: 'spotbugs',
      });
    }
  }

  return findings;
}

function sbSeverity(bugType: string, rank: number): Severity {
  if (bugType.startsWith('NP_')) return 'critical';
  if (rank <= 4) return 'critical';
  if (rank <= 9) return 'high';
  if (rank <= 14) return 'medium';
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
