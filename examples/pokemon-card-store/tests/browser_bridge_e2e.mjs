import http from 'node:http';
import net from 'node:net';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';

const projectDir = process.env.FISSION_PROJECT_DIR;
if (!projectDir) throw new Error('FISSION_PROJECT_DIR is required');

function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, '127.0.0.1', () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
    server.on('error', reject);
  });
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function waitForOutput(process, pattern, timeoutMs, label) {
  return new Promise((resolve, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error(`${label} did not become ready. Output:\n${output}`)), timeoutMs);
    function onData(chunk) {
      output += chunk.toString();
      if (pattern.test(output)) {
        clearTimeout(timer);
        resolve(output);
      }
    }
    process.stdout.on('data', onData);
    process.stderr.on('data', onData);
    process.once('exit', code => {
      clearTimeout(timer);
      reject(new Error(`${label} exited before ready with ${code}. Output:\n${output}`));
    });
  });
}

function httpText(url) {
  return new Promise((resolve, reject) => {
    http.get(url, response => {
      let data = '';
      response.setEncoding('utf8');
      response.on('data', chunk => data += chunk);
      response.on('end', () => resolve({ status: response.statusCode, headers: response.headers, body: data }));
    }).on('error', reject);
  });
}

function httpStatus(url) {
  return new Promise((resolve, reject) => {
    http.get(url, response => {
      response.resume();
      response.on('end', () => resolve({ status: response.statusCode, headers: response.headers }));
    }).on('error', reject);
  });
}

function chromePath() {
  if (process.env.FISSION_CHROME) return process.env.FISSION_CHROME;
  const candidates = [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/Applications/Chromium.app/Contents/MacOS/Chromium',
    'google-chrome',
    'chromium',
    'chromium-browser',
  ];
  for (const candidate of candidates) {
    if (candidate.includes('/') && fs.existsSync(candidate)) return candidate;
    if (!candidate.includes('/')) {
      const found = spawnSync('sh', ['-lc', `command -v ${candidate}`], { encoding: 'utf8' });
      if (found.status === 0 && found.stdout.trim()) return found.stdout.trim();
    }
  }
  throw new Error('Chrome/Chromium not found. Set FISSION_CHROME to the executable path.');
}

function wsFrame(data) {
  const payload = Buffer.from(data);
  const header = [0x81];
  if (payload.length < 126) {
    header.push(0x80 | payload.length);
  } else if (payload.length < 65536) {
    header.push(0x80 | 126, (payload.length >> 8) & 255, payload.length & 255);
  } else {
    throw new Error('CDP frame too large');
  }
  const mask = Buffer.from([1, 2, 3, 4]);
  const out = Buffer.alloc(payload.length);
  for (let i = 0; i < payload.length; i += 1) out[i] = payload[i] ^ mask[i % 4];
  return Buffer.concat([Buffer.from(header), mask, out]);
}

function connectWebSocket(wsUrl) {
  return new Promise((resolve, reject) => {
    const url = new URL(wsUrl);
    const key = Buffer.from(Math.random().toString()).toString('base64');
    const request = http.request({
      host: url.hostname,
      port: url.port,
      path: url.pathname + url.search,
      headers: {
        Connection: 'Upgrade',
        Upgrade: 'websocket',
        'Sec-WebSocket-Version': '13',
        'Sec-WebSocket-Key': key,
      },
    });
    request.on('upgrade', (_response, socket) => resolve(socket));
    request.on('error', reject);
    request.end();
  });
}

function cdpClient(socket) {
  let id = 0;
  let buffer = Buffer.alloc(0);
  const pending = new Map();
  const events = [];

  socket.on('data', chunk => {
    buffer = Buffer.concat([buffer, chunk]);
    while (buffer.length >= 2) {
      let length = buffer[1] & 127;
      let offset = 2;
      if (length === 126) {
        if (buffer.length < 4) return;
        length = buffer.readUInt16BE(2);
        offset = 4;
      } else if (length === 127) {
        if (buffer.length < 10) return;
        const big = buffer.readBigUInt64BE(2);
        if (big > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('CDP frame too large');
        length = Number(big);
        offset = 10;
      }
      if (buffer.length < offset + length) return;
      const payload = buffer.slice(offset, offset + length).toString();
      buffer = buffer.slice(offset + length);
      const message = JSON.parse(payload);
      if (message.id && pending.has(message.id)) {
        pending.get(message.id)(message);
        pending.delete(message.id);
      } else {
        events.push(message);
      }
    }
  });

  return {
    events,
    send(method, params = {}) {
      const message = { id: ++id, method, params };
      socket.write(wsFrame(JSON.stringify(message)));
      return new Promise(resolve => pending.set(id, resolve));
    },
    close() {
      socket.end();
    },
  };
}

async function waitForPage(cdpPort) {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const response = await httpText(`http://127.0.0.1:${cdpPort}/json`);
      const pages = JSON.parse(response.body);
      const page = pages.find(item => item.type === 'page');
      if (page?.webSocketDebuggerUrl) return page;
    } catch (_error) {
      // Chrome is still starting.
    }
    await sleep(250);
  }
  throw new Error('Chrome did not expose a debuggable page');
}

async function evaluate(client, expression) {
  const response = await client.send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (response.result.exceptionDetails) {
    throw new Error(JSON.stringify(response.result.exceptionDetails));
  }
  return response.result.result.value;
}

async function waitFor(client, expression, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await evaluate(client, expression);
    if (value) return value;
    await sleep(200);
  }
  const diagnostics = await evaluate(client, `({
    worker: document.querySelector('[data-fission-semantics="worker-status:catalog-filters"]')?.textContent,
    island: document.querySelector('[data-fission-semantics="island-status:cart-drawer"]')?.textContent,
    body: document.body.textContent.slice(-1000)
  })`).catch(error => ({ error: String(error) }));
  throw new Error(`${label} timed out: ${JSON.stringify(diagnostics)}; events=${JSON.stringify(client.events.slice(-20))}`);
}

const serverPort = await freePort();
const cdpPort = await freePort();
const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fission-pokemon-cdp-'));
let server;
let chrome;
let client;
let timeout;
function killProcessTree(child) {
  if (!child || child.killed) return;
  try {
    process.kill(-child.pid, 'SIGTERM');
  } catch (_error) {
    try { child.kill('SIGTERM'); } catch (_inner) {}
  }
}

function stopChildren() {
  if (client) client.close();
  killProcessTree(chrome);
  killProcessTree(server);
}
timeout = setTimeout(() => {
  stopChildren();
  console.error('browser bridge E2E timed out');
  process.exit(124);
}, 240000);

try {
  server = spawn('cargo', [
    'run', '-p', 'cargo-fission', '--bin', 'fission', '--',
    'server', 'serve', '--project-dir', projectDir,
    '--host', '127.0.0.1', '--port', String(serverPort),
  ], { cwd: process.cwd(), stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  await waitForOutput(server, /Serving Fission server app/, 120000, 'Fission server');

  const wasm = await httpStatus(`http://127.0.0.1:${serverPort}/assets/islands/cart-drawer.wasm`);
  if (wasm.status !== 200) throw new Error(`cart-drawer.wasm returned ${wasm.status}`);
  const runtime = await httpText(`http://127.0.0.1:${serverPort}/server-runtime.js`);
  if (runtime.status !== 200) throw new Error(`server-runtime.js returned ${runtime.status}`);
  if (!runtime.body.includes('fission_bridge_alloc')) throw new Error('server-runtime.js did not contain the bridge loader');

  chrome = spawn(chromePath(), [
    '--headless=new',
    '--disable-gpu',
    `--remote-debugging-port=${cdpPort}`,
    `--user-data-dir=${userDataDir}`,
    '--no-first-run',
    '--no-default-browser-check',
    `http://127.0.0.1:${serverPort}/`,
  ], { stdio: ['ignore', 'pipe', 'pipe'], detached: true });
  chrome.stdout.resume();
  chrome.stderr.resume();

  const page = await waitForPage(cdpPort);
  client = cdpClient(await connectWebSocket(page.webSocketDebuggerUrl));
  await client.send('Runtime.enable');
  await client.send('Log.enable');

  await waitFor(
    client,
    `document.querySelector('[data-fission-semantics="worker-status:catalog-filters"]')?.textContent === 'Worker bridge ready' && document.querySelector('[data-fission-semantics="island-status:cart-drawer"]')?.textContent === 'Island bridge ready'`,
    15000,
    'browser bridge boot',
  );

  await evaluate(client, `document.querySelector('form.fission-server-action-form button[type="submit"]')?.click()`);
  await waitFor(
    client,
    `location.pathname === '/' && document.body.textContent.includes('1 item in the server cart')`,
    15000,
    'server action redirect',
  );
  const afterServerAction = await evaluate(client, `({
    path: location.pathname,
    serverCart: document.body.textContent.includes('1 item in the server cart'),
    internalActionPath: location.pathname === '/__fission/action'
  })`);
  if (afterServerAction.internalActionPath || !afterServerAction.serverCart) {
    throw new Error(`server action did not redirect back to the route with updated state: ${JSON.stringify(afterServerAction)}`);
  }

  await waitFor(
    client,
    `document.querySelector('[data-fission-semantics="worker-status:catalog-filters"]')?.textContent === 'Worker bridge ready' && document.querySelector('[data-fission-semantics="island-status:cart-drawer"]')?.textContent === 'Island bridge ready'`,
    15000,
    'browser bridge reboot after server action',
  );

  const initial = await evaluate(client, `({
    count: document.querySelector('[data-fission-semantics="island-cart-count"]')?.textContent,
    total: document.querySelector('[data-fission-semantics="island-cart-total"]')?.textContent,
    line: document.querySelector('[data-fission-semantics="island-cart-line"]')?.textContent,
    actionRole: document.querySelector('[data-fission-semantics="island-action:add-card"]')?.getAttribute('role')
  })`);
  if (initial.count !== '0 items in the browser island cart') throw new Error(`unexpected initial count: ${JSON.stringify(initial)}`);
  if (initial.total !== '£0.00') throw new Error(`unexpected initial total: ${JSON.stringify(initial)}`);
  if (initial.actionRole !== 'button') throw new Error(`island action was not bound: ${JSON.stringify(initial)}`);

  await evaluate(client, `document.querySelector('[data-fission-semantics="island-action:add-card"]')?.click()`);

  const afterClick = await waitFor(
    client,
    `(() => {
      const count = document.querySelector('[data-fission-semantics="island-cart-count"]')?.textContent;
      if (count !== '1 item in the browser island cart') return null;
      return {
        count,
        short: document.querySelector('[data-fission-semantics="island-cart-count-short"]')?.textContent,
        total: document.querySelector('[data-fission-semantics="island-cart-total"]')?.textContent,
        line: document.querySelector('[data-fission-semantics="island-cart-line"]')?.textContent,
        status: document.querySelector('[data-fission-semantics="cart-drawer"] [data-fission-semantics="island-status:cart-drawer"]')?.textContent,
        lineColor: getComputedStyle(document.querySelector('[data-fission-semantics="island-cart-line"] .fission-site-text-run') || document.querySelector('[data-fission-semantics="island-cart-line"]')).color,
      };
    })()`,
    10000,
    'island click update',
  );

  if (afterClick.short !== '1') throw new Error(`short count was not updated: ${JSON.stringify(afterClick)}`);
  if (afterClick.total !== '£249.00') throw new Error(`subtotal was not updated: ${JSON.stringify(afterClick)}`);
  if (!afterClick.line.includes('Charizard Holo')) throw new Error(`line item was not updated: ${JSON.stringify(afterClick)}`);
  if (!afterClick.status.includes('reducer event')) throw new Error(`status was not updated: ${JSON.stringify(afterClick)}`);
  if (afterClick.lineColor === 'rgb(0, 0, 0)') throw new Error(`bridge text update lost generated text styling: ${JSON.stringify(afterClick)}`);

  await waitFor(
    client,
    `(() => {
      const action = document.querySelector('[data-fission-semantics="island-action:add-card"]');
      if (!action) return null;
      let node = action.parentElement;
      while (node) {
        const actionRect = action.getBoundingClientRect();
        const nodeRect = node.getBoundingClientRect();
        if (node.scrollHeight > node.clientHeight && actionRect.bottom > nodeRect.bottom) {
          node.scrollTop += actionRect.bottom - nodeRect.bottom + 24;
        }
        if (node.scrollHeight > node.clientHeight && actionRect.top < nodeRect.top) {
          node.scrollTop -= nodeRect.top - actionRect.top + 24;
        }
        node = node.parentElement;
      }
      action.scrollIntoView({ block: 'center', inline: 'center' });
      const rect = action.getBoundingClientRect();
      return rect.top >= 0 && rect.bottom <= window.innerHeight ? true : null;
    })()`,
    10000,
    'island screenshot target',
  );
  await sleep(200);
  const outputDir = path.join(projectDir, 'target/fission/e2e');
  fs.mkdirSync(outputDir, { recursive: true });
  const screenshot = await client.send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(path.join(outputDir, 'pokemon-browser-bridge.png'), Buffer.from(screenshot.result.data, 'base64'));
} finally {
  clearTimeout(timeout);
  stopChildren();
  try {
    fs.rmSync(userDataDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  } catch (_error) {
    // Chrome can briefly keep profile files open after SIGTERM. The OS temp cleaner will remove leftovers.
  }
}
