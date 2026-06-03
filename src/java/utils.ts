import fs from 'fs';
import path from 'path';

export function findReports(cwd: string, name: string): string[] {
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

export function toArray<T>(val: T | T[] | null | undefined): T[] {
  if (Array.isArray(val)) return val;
  if (val != null) return [val];
  return [];
}
