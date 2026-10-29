// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {copyFile, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

test('export preflight selects explicit, environment, bundled and PATH Pandoc in order', {skip: process.platform !== 'linux'}, async () => {
  const source = path.dirname(fileURLToPath(import.meta.url));
  const dir = await mkdtemp(path.join(tmpdir(), 'mdcard-pandoc-resolution-'));
  try {
    const root = path.join(dir, 'renderer');
    await mkdir(root);
    for (const name of await readdir(source)) {
      if (name.endsWith('.mjs')) await copyFile(path.join(source, name), path.join(root, name));
    }
    await symlink(path.join(source, 'node_modules'), path.join(root, 'node_modules'), 'dir');
    const chrome = path.join(root, 'runtime/linux/chromium/chrome');
    await mkdir(path.dirname(chrome), {recursive: true});
    await writeFile(chrome, 'preflight only');
    const marker = path.join(dir, 'invoked');
    const explicit = path.join(dir, 'explicit-pandoc');
    const configured = path.join(dir, 'configured-pandoc');
    const bundled = path.join(root, 'runtime/pandoc/pandoc');
    const devBundle = path.join(root, 'runtime/linux/pandoc/pandoc');
    const fallback = path.join(dir, 'bin/pandoc');
    for (const [file, name] of [[explicit, 'explicit'], [configured, 'environment'], [bundled, 'bundled'], [devBundle, 'dev-bundle'], [fallback, 'path']]) {
      await mkdir(path.dirname(file), {recursive: true});
      await writeFile(file, `#!/bin/sh\nprintf '%s' '${name}' > "$MDCARD_TEST_MARKER"\nexit 42\n`, {mode: 0o755});
    }
    const request = path.join(dir, 'request.json');
    for (const [override, envPath, expected, format] of [
      [explicit, configured, 'explicit', 'docx'],
      ['', configured, 'environment', 'epub'],
      ['', '', 'bundled', 'docx'],
      ['', '', 'dev-bundle', 'docx'],
      ['', '', 'path', 'epub'],
    ]) {
      if (expected === 'dev-bundle') await rm(bundled);
      if (expected === 'path') await rm(devBundle);
      await rm(marker, {force: true});
      await writeFile(request, JSON.stringify({source: '# test', output: path.join(dir, 'out.' + format), format, options: {pandocPath: override}}));
      const child = spawnSync(process.execPath, [path.join(root, 'export.mjs'), request], {
        env: {...process.env, MDCARD_PANDOC: envPath, MDCARD_TEST_MARKER: marker, PATH: path.join(dir, 'bin')},
        encoding: 'utf8', timeout: 30000,
      });
      assert.ifError(child.error);
      assert.equal(child.status, 1, child.stderr);
      assert.ok(child.stdout.trim(), `renderer produced no events: ${child.stderr}`);
      const events = child.stdout.trim().split('\n').map(line => JSON.parse(line));
      assert.equal(events.at(-1).stage, 'convert', `${child.stdout}\n${child.stderr}`);
      assert.match(events.at(-1).message, /exited 42/);
      assert.equal(await readFile(marker, 'utf8'), expected);
    }
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});
