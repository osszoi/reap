import type { Finding, Hotspot, Config } from './types.js';

const DEAD_CODE_RULES = new Set([
  'UnusedLocalVariable', 'UnusedPrivateField', 'UnusedPrivateMethod',
  'UnusedFormalParameter', 'UnnecessaryImport', 'UnusedImports',
]);

const COMPLEXITY_RULES = new Set([
  'CyclomaticComplexity', 'CognitiveComplexity', 'NPathComplexity',
]);

type Checker = (findings: Finding[], hotspots: Hotspot[], config: Config) => string | null;

function plural(n: number, noun: string): string {
  return `${n} ${noun}${n > 1 ? 's' : ''}`;
}

const CHECKERS: Record<string, Checker> = {
  nullpointers: (findings) => {
    const n = findings.filter(f => f.source === 'spotbugs' && f.rule.startsWith('NP_')).length;
    return n > 0 ? `${plural(n, 'null pointer path')} found` : null;
  },
  complexity: (findings) => {
    const n = findings.filter(f => f.source === 'pmd' && COMPLEXITY_RULES.has(f.rule)).length;
    return n > 0 ? plural(n, 'complexity violation') : null;
  },
  'dead-code': (findings) => {
    const n = findings.filter(f => f.source === 'pmd' && DEAD_CODE_RULES.has(f.rule)).length;
    return n > 0 ? plural(n, 'dead code issue') : null;
  },
  bugs: (findings) => {
    const n = findings.filter(f => f.source === 'spotbugs' && f.severity !== 'low').length;
    return n > 0 ? plural(n, 'SpotBugs finding') : null;
  },
  hotspots: (_, hotspots, config) => {
    if (config.maxHotspotScore === Infinity) return null;
    const n = hotspots.filter(h => h.score > config.maxHotspotScore).length;
    return n > 0 ? `${plural(n, 'file')} exceed hotspot score ${config.maxHotspotScore}` : null;
  },
  all: (findings) => {
    const n = findings.filter(f => f.severity === 'critical' || f.severity === 'high').length;
    return n > 0 ? plural(n, 'critical/high finding') : null;
  },
};

export function checkThresholds(
  config: Config,
  findings: Finding[],
  hotspots: Hotspot[]
): { passed: boolean; reasons: string[] } {
  const reasons = config.failOn
    .map(t => CHECKERS[t]?.(findings, hotspots, config) ?? null)
    .filter((r): r is string => r !== null);
  return { passed: reasons.length === 0, reasons };
}
