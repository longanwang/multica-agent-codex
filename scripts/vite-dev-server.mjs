import http from 'node:http';
import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const desktopRoot = path.join(repoRoot, 'apps', 'desktop');
const port = 1420;
const host = '127.0.0.1';
const devUrl = `http://${host}:${port}/`;
let viteChild = null;

const existing = await readExistingServer();
if (existing.up && existing.body.includes('<title>Multica</title>')) {
  console.log(`复用已有 Multica Vite dev server: ${devUrl}`);
  watchReusedServer();
} else if (existing.up) {
  console.error(`端口 ${port} 已被其他服务占用，请先关闭该服务后再启动 Multica。`);
  process.exit(1);
} else {
  startVite();
}

function startVite() {
  const viteBin = path.join(repoRoot, 'node_modules', 'vite', 'bin', 'vite.js');
  viteChild = spawn(process.execPath, [viteBin, '--host', host, '--port', String(port), '--strictPort'], {
    cwd: desktopRoot,
    stdio: 'inherit',
  });

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => {
      viteChild?.kill(signal);
    });
  }

  viteChild.on('exit', (code, signal) => {
    if (signal) {
      process.exit(1);
      return;
    }
    process.exit(code ?? 0);
  });
}

function watchReusedServer() {
  const interval = setInterval(async () => {
    const current = await readExistingServer();
    if (!current.up) {
      clearInterval(interval);
      console.warn(`已复用的 Multica Vite dev server 已退出，重新启动: ${devUrl}`);
      startVite();
    }
  }, 1000);
}

function readExistingServer() {
  return new Promise((resolve) => {
    const request = http.get(devUrl, { timeout: 800 }, (response) => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => {
        body += chunk;
        if (body.length > 2048) {
          request.destroy();
        }
      });
      response.on('end', () => {
        resolve({ up: true, body });
      });
    });

    request.on('timeout', () => {
      request.destroy();
      resolve({ up: false, body: '' });
    });
    request.on('error', () => {
      resolve({ up: false, body: '' });
    });
  });
}
