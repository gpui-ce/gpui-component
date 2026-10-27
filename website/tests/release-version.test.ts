import { expect, test } from 'bun:test';
import { join } from 'node:path';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';

for (const [label, variables, expected] of [
  ['release tag', { PUBLIC_SITE_VERSION: 'v0.8.1', GPUI_KIT_VERSION: '' }, '0.8.1'],
  ['main deployment', { PUBLIC_SITE_VERSION: 'main', GPUI_KIT_VERSION: '0.8.2' }, '0.8.2'],
  ['historical deployment', { PUBLIC_SITE_VERSION: 'v0.6.6', GPUI_KIT_VERSION: '0.6.6' }, '0.6.6'],
] as const) {
  test(`documentation variables follow the ${label}`, () => {
    const result = Bun.spawnSync(['bun', '--eval', `
      const { expandDocVariables } = await import('./src/lib/doc-variables.js');
      console.log(expandDocVariables('gpui-kit = "{{gpui_kit_version}}"'));
    `], { cwd: join(import.meta.dir, '..'), env: { ...process.env, ...variables } });
    expect(result.exitCode).toBe(0);
    expect(result.stdout.toString().trim()).toBe(`gpui-kit = "${expected}"`);
  });
}

for (const [label, manifest, expected] of [
  ['explicit package version', '[package]\nversion = "1.2.3"', '1.2.3'],
  ['inherited workspace version', '[package]\nversion.workspace = true', '4.5.6'],
] as const) {
  test(`local documentation resolves an ${label}`, () => {
    const directory = mkdtempSync(join(tmpdir(), 'gpui-doc-version-'));
    try {
      mkdirSync(join(directory, 'crates', 'kit'), { recursive: true });
      mkdirSync(join(directory, 'website'));
      writeFileSync(join(directory, 'crates', 'kit', 'Cargo.toml'), manifest);
      writeFileSync(join(directory, 'Cargo.toml'), '[workspace.package]\nversion = "4.5.6"\n[workspace.dependencies]\n');
      const source = pathToFileURL(join(import.meta.dir, '../src/lib/doc-variables.js')).href;
      const result = Bun.spawnSync([process.execPath, '--eval', `
        const { docVariable } = await import(${JSON.stringify(source)});
        console.log(docVariable('gpui_kit_version'));
      `], {
        cwd: join(directory, 'website'),
        env: { ...process.env, PUBLIC_SITE_VERSION: '', GPUI_KIT_VERSION: '' },
      });
      expect(result.exitCode).toBe(0);
      expect(result.stdout.toString().trim()).toBe(expected);
    } finally {
      rmSync(directory, { recursive: true });
    }
  });
}
