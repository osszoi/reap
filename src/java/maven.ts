import { execSync, spawnSync } from 'child_process';

export function isMavenAvailable(): boolean {
  const r = spawnSync('mvn', ['--version'], { encoding: 'utf8' });
  return r.status === 0;
}

export function compile(cwd: string): void {
  execSync('mvn compile --no-transfer-progress -q', {
    cwd,
    maxBuffer: 50 * 1024 * 1024,
    encoding: 'utf8',
    stdio: 'inherit',
  });
}
