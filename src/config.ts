import type { Config } from './types.js';

export const DEFAULTS: Omit<Config, 'sub' | 'cwd'> = {
  failOn: ['nullpointers'],
  maxComplexity: 20,
  maxHotspotScore: Infinity,
  noCompile: false,
  verbose: false,
  top: 20,
};

export function parseFailOn(raw: string): string[] {
  return raw.split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
}
