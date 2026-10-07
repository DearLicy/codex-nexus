import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const node = process.execPath;

function waitForReady(child) {
  return new Promise((resolve, reject) => {
    let buffer = '';
    const onData = (chunk) => {
      buffer += chunk.toString();
      const line = buffer.split('\n').find((value) => value.includes('"ready":true'));
      if (line) {
        child.stdout.off('data', onData);
        resolve(JSON.parse(line));
      }
    };
    child.stdout.on('data', onData);
    child.once('error', reject);
    child.once('exit', (code) => code && reject(new Error(`gateway exited with ${code}`)));
  });
}

test('gateway exposes health, models, auth and strict namespace routing', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'codex-nexus-'));
  const config = path.join(dir, 'gateway.json');
  await writeFile(config, JSON.stringify({
    routes: [{ id: 'demo', namespace: 'demo', providerId: 'demo', baseUrl: 'https://example.invalid', models: ['model-a'], default: true }],
    accounts: [{ id: 'account-a', providerId: 'demo', quotaRemaining: 100, apiKey: 'test' }],
    apiKeys: [{ id: 'local', key: 'local-test' }],
  }));
  const child = spawn(node, [path.join(root, 'packages/gateway/server.mjs')], { env: { ...process.env, CODEX_NEXUS_CONFIG: config, CODEX_NEXUS_PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
  try {
    const ready = await waitForReady(child);
    assert.equal((await fetch(`http://127.0.0.1:${ready.port}/health`)).status, 200);
    const models = await fetch(`http://127.0.0.1:${ready.port}/v1/models`, { headers: { authorization: 'Bearer local-test' } });
    assert.deepEqual((await models.json()).data[0].id, 'demo/model-a');
    const unknown = await fetch(`http://127.0.0.1:${ready.port}/v1/responses`, { method: 'POST', headers: { authorization: 'Bearer local-test', 'content-type': 'application/json' }, body: JSON.stringify({ model: 'missing/model' }) });
    assert.equal(unknown.status, 404);
  } finally {
    child.kill('SIGTERM');
    await rm(dir, { recursive: true, force: true });
  }
});

test('image MCP advertises native fallback tools over stdio', async () => {
  const child = spawn(node, [path.join(root, 'packages/image-mcp/server.mjs')], { stdio: ['pipe', 'pipe', 'pipe'] });
  const output = new Promise((resolve, reject) => {
    let text = '';
    child.stdout.on('data', (chunk) => {
      text += chunk.toString();
      if (text.includes('"tools"')) resolve(text);
    });
    child.once('error', reject);
    setTimeout(() => reject(new Error('MCP response timeout')), 3000);
  });
  child.stdin.write('{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}\n');
  const response = JSON.parse((await output).trim());
  assert.deepEqual(response.result.tools.map((tool) => tool.name), ['generate_image', 'edit_image', 'list_image_models', 'get_image_job']);
  child.kill('SIGTERM');
});
