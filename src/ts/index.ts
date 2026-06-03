import { spawnSync } from 'child_process';

export function runFallow(args: string[]): never {
  const result = spawnSync('fallow', args, { stdio: 'inherit' });
  if (result.error) {
    if ((result.error as NodeJS.ErrnoException).code === 'ENOENT') {
      console.error("  error: 'fallow' not found — install it with: npm i -g fallow");
      process.exit(2);
    }
    throw result.error;
  }
  process.exit(result.status ?? 0);
}
