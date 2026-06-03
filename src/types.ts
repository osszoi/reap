export type Severity = 'critical' | 'high' | 'medium' | 'low';

export interface Finding {
  file: string;
  line: string;
  rule: string;
  ruleset: string;
  severity: Severity;
  message: string;
  url?: string;
  source: 'pmd' | 'spotbugs';
}

export interface Hotspot {
  file: string;
  score: number;
  weightedCommits: number;
  totalCommits: number;
  loc: number;
  churnLines: number;
}

export type Subcommand = 'all' | 'hotspots' | 'dead-code' | 'complexity' | 'bugs';

export interface Config {
  sub: Subcommand;
  failOn: string[];
  maxComplexity: number;
  maxHotspotScore: number;
  noCompile: boolean;
  verbose: boolean;
  top: number;
  cwd: string;
}
