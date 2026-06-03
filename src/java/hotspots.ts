import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import type { Hotspot } from '../types.js';

const HALF_LIFE = 90 * 24 * 3600;

export function analyzeHotspots(cwd: string): Hotspot[] {
  const raw = execSync("git log --numstat --pretty='format:COMMIT %H %at' --no-merges", {
    cwd,
    maxBuffer: 100 * 1024 * 1024,
    encoding: 'utf8',
  });

  const stats = new Map<string, { decays: number[]; added: number; deleted: number }>();
  const now = Date.now() / 1000;
  let ts = 0;

  for (const line of raw.split('\n')) {
    if (line.startsWith('COMMIT ')) {
      ts = parseInt(line.split(' ')[2] ?? '0', 10);
    } else if (ts && line.includes('\t')) {
      const parts = line.split('\t');
      if (parts.length !== 3 || parts[0] === '-' || parts[0] === '') continue;
      const filePath = parts[2]!;
      if (!filePath.endsWith('.java')) continue;
      const added = parseInt(parts[0], 10);
      const deleted = parseInt(parts[1], 10);
      if (isNaN(added) || isNaN(deleted)) continue;
      const decay = Math.exp(-(now - ts) * Math.LN2 / HALF_LIFE);
      const s = stats.get(filePath) ?? { decays: [], added: 0, deleted: 0 };
      s.decays.push(decay);
      s.added += added;
      s.deleted += deleted;
      stats.set(filePath, s);
    }
  }

  const results: Hotspot[] = [];
  for (const [filePath, s] of stats) {
    const loc = countLines(path.join(cwd, filePath));
    const weightedCommits = s.decays.reduce((a, b) => a + b, 0);
    const score = weightedCommits * (1 + 0.1 * Math.log(Math.max(loc, 1)));
    results.push({
      file: filePath,
      score: Math.round(score * 100) / 100,
      weightedCommits: Math.round(weightedCommits * 100) / 100,
      totalCommits: s.decays.length,
      loc,
      churnLines: s.added + s.deleted,
    });
  }

  return results.sort((a, b) => b.score - a.score);
}

function countLines(filePath: string): number {
  if (!fs.existsSync(filePath)) return 0;
  try {
    return fs.readFileSync(filePath, 'utf8').split('\n').length;
  } catch {
    return 0;
  }
}
