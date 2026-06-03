import { execSync } from 'child_process';
import { XMLParser } from 'fast-xml-parser';
import fs from 'fs';
import type { Finding, Severity } from '../types.js';
import { findReports, toArray } from './utils.js';

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
  } catch { /* spotbugs exits non-zero when bugs found — reports still written */ }
  return collectSpotbugsReports(cwd);
}

function collectSpotbugsReports(cwd: string): Finding[] {
  const xmlFiles = findReports(cwd, 'spotbugsXml.xml');
  const parser = new XMLParser({ ignoreAttributes: false, attributeNamePrefix: '@_', processEntities: false });
  const findings: Finding[] = [];
  for (const xmlPath of xmlFiles) {
    const doc = parser.parse(fs.readFileSync(xmlPath, 'utf8'));
    for (const bug of toArray<Record<string, unknown>>(doc?.BugCollection?.BugInstance)) {
      const f = parseBugInstance(bug);
      if (f) findings.push(f);
    }
  }
  return findings;
}

type BugAttrs = { bugType: string; rank: number; category: string };
type SrcLocation = { file: string; line: string };

function readBugAttrs(bug: Record<string, unknown>): BugAttrs {
  return {
    bugType:  String(bug['@_type'] ?? ''),
    rank:     parseInt(String(bug['@_rank'] ?? '20'), 10),
    category: String(bug['@_category'] ?? ''),
  };
}

// fallow-ignore-next-line complexity
function readSrcLocation(bug: Record<string, unknown>): SrcLocation {
  const srcLines = toArray<Record<string, string>>(bug.SourceLine as Record<string, string>[]);
  const src = srcLines.find(s => s['@_primary'] === 'true') ?? srcLines[0];
  return {
    file: String(src?.['@_sourcepath'] ?? '?'),
    line: String(src?.['@_start'] ?? '?'),
  };
}

function parseBugInstance(bug: Record<string, unknown>): Finding | null {
  const { bugType, rank, category } = readBugAttrs(bug);
  if (category === 'MALICIOUS_CODE' && rank > 14 && !bugType.startsWith('NP_')) return null;
  const { file, line } = readSrcLocation(bug);
  const msg = bug.LongMessage ?? bug['@_type'] ?? '?';
  return {
    file,
    line,
    rule: bugType,
    ruleset: category,
    severity: sbSeverity(bugType, rank),
    message: (typeof msg === 'string' ? msg : String(msg)).trim(),
    source: 'spotbugs',
  };
}

// fallow-ignore-next-line complexity
function sbSeverity(bugType: string, rank: number): Severity {
  if (bugType.startsWith('NP_')) return 'critical';
  if (rank <= 4) return 'critical';
  if (rank <= 9) return 'high';
  if (rank <= 14) return 'medium';
  return 'low';
}
