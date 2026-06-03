import type { Finding, Hotspot, Config } from './types.js';

export function checkThresholds(
  config: Config,
  findings: Finding[],
  hotspots: Hotspot[]
): { passed: boolean; reasons: string[] } {
  const reasons: string[] = [];

  for (const target of config.failOn) {
    switch (target) {
      case 'nullpointers': {
        const npe = findings.filter(f => f.source === 'spotbugs' && f.rule.startsWith('NP_'));
        if (npe.length > 0)
          reasons.push(`${npe.length} null pointer path${npe.length > 1 ? 's' : ''} found`);
        break;
      }
      case 'complexity': {
        const cx = findings.filter(f =>
          f.source === 'pmd' &&
          ['CyclomaticComplexity', 'CognitiveComplexity', 'NPathComplexity'].includes(f.rule)
        );
        if (cx.length > 0)
          reasons.push(`${cx.length} complexity violation${cx.length > 1 ? 's' : ''}`);
        break;
      }
      case 'dead-code': {
        const dead = findings.filter(f =>
          f.source === 'pmd' &&
          ['UnusedLocalVariable','UnusedPrivateField','UnusedPrivateMethod',
           'UnusedFormalParameter','UnnecessaryImport','UnusedImports'].includes(f.rule)
        );
        if (dead.length > 0)
          reasons.push(`${dead.length} dead code issue${dead.length > 1 ? 's' : ''}`);
        break;
      }
      case 'bugs': {
        const bugs = findings.filter(f => f.source === 'spotbugs' && f.severity !== 'low');
        if (bugs.length > 0)
          reasons.push(`${bugs.length} SpotBugs finding${bugs.length > 1 ? 's' : ''}`);
        break;
      }
      case 'hotspots': {
        if (config.maxHotspotScore !== Infinity) {
          const hot = hotspots.filter(h => h.score > config.maxHotspotScore);
          if (hot.length > 0)
            reasons.push(`${hot.length} file${hot.length > 1 ? 's' : ''} exceed hotspot score ${config.maxHotspotScore}`);
        }
        break;
      }
      case 'all': {
        const crit = findings.filter(f => f.severity === 'critical' || f.severity === 'high');
        if (crit.length > 0)
          reasons.push(`${crit.length} critical/high finding${crit.length > 1 ? 's' : ''}`);
        break;
      }
    }
  }

  return { passed: reasons.length === 0, reasons };
}
