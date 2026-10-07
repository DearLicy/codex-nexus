#!/usr/bin/env node
/** Minimal MCP stdio server for Codex Nexus image generation. */
import readline from 'node:readline';
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';

const gateway = process.env.CODEX_NEXUS_GATEWAY || 'http://127.0.0.1:8787';
const gatewayKey = process.env.CODEX_NEXUS_API_KEY || '';
const outputDir = process.env.CODEX_NEXUS_IMAGE_DIR || path.join(process.env.CODEX_NEXUS_DATA || '.', 'generated-images');
const jobs = new Map();

const tools = [
  { name: 'generate_image', description: 'Generate an image through the configured Codex Nexus image pool.', inputSchema: { type: 'object', properties: { prompt: { type: 'string' }, model: { type: 'string' }, size: { type: 'string' }, quality: { type: 'string' }, sessionId: { type: 'string' } }, required: ['prompt'] } },
  { name: 'edit_image', description: 'Edit an image through the configured Codex Nexus image pool.', inputSchema: { type: 'object', properties: { imagePath: { type: 'string' }, prompt: { type: 'string' }, model: { type: 'string' }, sessionId: { type: 'string' } }, required: ['imagePath', 'prompt'] } },
  { name: 'list_image_models', description: 'List configured image models.', inputSchema: { type: 'object', properties: {} } },
  { name: 'get_image_job', description: 'Read the status of an image job.', inputSchema: { type: 'object', properties: { jobId: { type: 'string' } }, required: ['jobId'] } },
];

function reply(id, result) { process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`); }
function failure(id, message, code = 'image_mcp_error') { reply(id, { isError: true, content: [{ type: 'text', text: `${code}: ${message}` }] }); }

async function callGateway(endpoint, body) {
  const response = await fetch(`${gateway}${endpoint}`, { method: 'POST', headers: { 'content-type': 'application/json', ...(gatewayKey ? { authorization: `Bearer ${gatewayKey}` } : {}) }, body: JSON.stringify(body) });
  const text = await response.text();
  let data; try { data = JSON.parse(text); } catch { data = { raw: text }; }
  if (!response.ok) throw new Error(data?.error?.message || `gateway returned ${response.status}`);
  return data;
}

async function saveImage(jobId, item) {
  const encoded = item.b64_json || item.base64;
  if (!encoded) return null;
  await fs.mkdir(outputDir, { recursive: true });
  const file = path.join(outputDir, `${jobId}.png`);
  await fs.writeFile(file, Buffer.from(encoded, 'base64'));
  return file;
}

async function invoke(name, args) {
  const jobId = crypto.randomUUID?.() || `job-${Date.now()}`;
  if (name === 'list_image_models') {
    const response = await fetch(`${gateway}/v1/models`, { headers: gatewayKey ? { authorization: `Bearer ${gatewayKey}` } : {} });
    const data = await response.json();
    return [{ type: 'text', text: JSON.stringify(data.data.filter((m) => /image|dall|flux|gemini/i.test(m.id))) }];
  }
  if (name === 'get_image_job') {
    const job = jobs.get(args.jobId);
    if (!job) throw new Error('image job not found');
    return [{ type: 'text', text: JSON.stringify(job) }];
  }
  const job = { id: jobId, status: 'running', model: args.model || 'default', startedAt: new Date().toISOString() };
  jobs.set(jobId, job);
  try {
    let request = { ...args };
    if (name === 'edit_image') {
      const source = await fs.readFile(args.imagePath);
      request = { ...request, image: source.toString('base64'), imagePath: undefined };
    }
    const data = await callGateway(name === 'edit_image' ? '/v1/images/edits' : '/v1/images/generations', request);
    const item = data.data?.[0] || data.output?.[0] || {};
    const file = await saveImage(jobId, item);
    job.status = 'completed'; job.outputPath = file || item.url || null; job.completedAt = new Date().toISOString();
    return [{ type: 'text', text: JSON.stringify(job) }, ...(item.b64_json ? [{ type: 'image', data: item.b64_json, mimeType: 'image/png' }] : [])];
  } catch (err) {
    job.status = 'failed'; job.error = err.message; job.completedAt = new Date().toISOString();
    throw err;
  }
}

const rl = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of rl) {
  if (!line.trim()) continue;
  let request; try { request = JSON.parse(line); } catch { continue; }
  if (request.method === 'initialize') reply(request.id, { protocolVersion: '2024-11-05', capabilities: { tools: {} }, serverInfo: { name: 'codex-nexus-image', version: '0.1.0' } });
  else if (request.method === 'notifications/initialized') continue;
  else if (request.method === 'tools/list') reply(request.id, { tools });
  else if (request.method === 'tools/call') {
    try { reply(request.id, { content: await invoke(request.params?.name, request.params?.arguments || {}) }); }
    catch (err) { failure(request.id, err.message); }
  } else if (request.id !== undefined) failure(request.id, `Unsupported method: ${request.method}`, 'method_not_found');
}
