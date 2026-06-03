import { execSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import type { Hotspot } from '../types.js';

const HALF_LIFE = 90 * 24 * 3600;

type FileStats = { decays: number[]; added: number; deleted: number };

// fallow-ignore-next-line complexity
function parseNumstatLine(line: string): { filePath: string; added: number; deleted: number } | null {
  if (!line.includes('\t')) return null;
  const parts = line.split('\t');
  if (parts.length !== 3 || parts[0] === '-' || parts[0] === '') return null;
  const filePath = parts[2]!;
  if (!filePath.endsWith('.java')) return null;
  const added = parseInt(parts[0], 10);
  const deleted = parseInt(parts[1], 10);
  if (isNaN(added) || isNaN(deleted)) return null;
  return { filePath, added, deleted };
}

function parseGitLog(raw: string): Map<string, FileStats> {
  const stats = new Map<string, FileStats>();
  const now = Date.now() / 1000;
  let ts = 0;

  for (const line of raw.split('\n')) {
    if (line.startsWith('COMMIT ')) {
      ts = parseInt(line.split(' ')[2] ?? '0', 10);
      continue;
    }
    if (!ts) continue;
    const entry = parseNumstatLine(line);
    if (!entry) continue;
    const decay = Math.exp(-(now - ts) * Math.LN2 / HALF_LIFE);
    const s = stats.get(entry.filePath) ?? { decays: [], added: 0, deleted: 0 };
    s.decays.push(decay);
    s.added += entry.added;
    s.deleted += entry.deleted;
    stats.set(entry.filePath, s);
  }

  return stats;
}

function scoreHotspot(filePath: string, s: FileStats, cwd: string): Hotspot {
  const loc = countLines(path.join(cwd, filePath));
  const weightedCommits = s.decays.reduce((a, b) => a + b, 0);
  const score = weightedCommits * (1 + 0.1 * Math.log(Math.max(loc, 1)));
  return {
    file: filePath,
    score: Math.round(score * 100) / 100,
    weightedCommits: Math.round(weightedCommits * 100) / 100,
    totalCommits: s.decays.length,
    loc,
    churnLines: s.added + s.deleted,
  };
}

export function analyzeHotspots(cwd: string): Hotspot[] {
  const raw = execSync("git log --numstat --pretty='format:COMMIT %H %at' --no-merges", {
    cwd,
    maxBuffer: 100 * 1024 * 1024,
    encoding: 'utf8',
  });
  return Array.from(parseGitLog(raw).entries())
    .map(([filePath, s]) => scoreHotspot(filePath, s, cwd))
    .sort((a, b) => b.score - a.score);
}

function countLines(filePath: string): number {
  try {
    return fs.readFileSync(filePath, 'utf8').split('\n').length;
  } catch {
    return 0;
  }
}
