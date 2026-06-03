import fs from 'fs';
import path from 'path';

export type ProjectLang = 'java' | 'ts' | 'both' | 'unknown';

export interface ProjectInfo {
  lang: ProjectLang;
  root: string;
  name: string;
}

export function detectProject(cwd: string): ProjectInfo {
  const javaRoot = findUpwards('pom.xml', cwd);
  const tsRoot =
    fs.existsSync(path.join(cwd, 'package.json')) ||
    fs.existsSync(path.join(cwd, 'tsconfig.json'));

  const root = javaRoot ?? cwd;
  const name = path.basename(root);

  if (javaRoot && tsRoot) return { lang: 'both', root, name };
  if (javaRoot) return { lang: 'java', root: javaRoot, name };
  if (tsRoot) return { lang: 'ts', root: cwd, name };
  return { lang: 'unknown', root: cwd, name };
}

function findUpwards(filename: string, from: string): string | null {
  let dir = from;
  for (let i = 0; i < 6; i++) {
    if (fs.existsSync(path.join(dir, filename))) return dir;
    const parent = path.dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}
