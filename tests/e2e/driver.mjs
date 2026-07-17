// Generic headless-Chrome driver for the printpdf wasm demo e2e tests.
// Usage: node driver.mjs --root <dir-to-serve> --chrome <path-to-chrome> \
//          --scenarios <scenarios.mjs> --out <artifact-dir>
//
// Serves <dir> on an ephemeral port, opens the page, records EVERY console
// message / pageerror / requestfailed, runs each scenario from the scenario
// module ({name, run(page, ctx)}), captures a screenshot per scenario, and
// exits nonzero if any scenario throws or any severe console error occurred.
// Downloaded/generated PDFs are written into --out for external validation
// (pdftoppm/pdffonts) by the caller.

import http from 'node:http';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { extname, join, resolve } from 'node:path';
import puppeteer from 'puppeteer-core';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((acc, a, i, arr) => {
    if (a.startsWith('--')) acc.push([a.slice(2), arr[i + 1]]);
    return acc;
  }, []),
);

const MIME = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.wasm': 'application/wasm', '.css': 'text/css', '.json': 'application/json',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.svg': 'image/svg+xml',
  '.pdf': 'application/pdf', '.ttf': 'font/ttf', '.otf': 'font/otf',
};

async function serve(root) {
  const server = http.createServer(async (req, res) => {
    try {
      const path = join(root, req.url === '/' ? 'index.html' : decodeURIComponent(req.url.split('?')[0]));
      const body = await readFile(path);
      res.writeHead(200, { 'content-type': MIME[extname(path)] ?? 'application/octet-stream' });
      res.end(body);
    } catch {
      res.writeHead(404); res.end('not found');
    }
  });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  return { server, url: `http://127.0.0.1:${server.address().port}/` };
}

const out = resolve(args.out ?? 'e2e-artifacts');
await mkdir(out, { recursive: true });

const { server, url } = await serve(resolve(args.root));
const browser = await puppeteer.launch({
  executablePath: args.chrome,
  headless: true,
  // Generous protocol timeout: a busy wasm page (multi-MB SVG injection) can
  // stall the renderer long enough for default CDP timeouts to fire.
  protocolTimeout: 180000,
  args: ['--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
});

const consoleLog = [];
let failures = 0;

try {
  const page = await browser.newPage();
  // Realistic desktop viewport: at the 800x600 default the stacked mobile
  // layout applies and scenario clicks exercise a layout users rarely see.
  await page.setViewport({ width: 1440, height: 900 });
  page.on('console', m => consoleLog.push({ type: m.type(), text: m.text() }));
  page.on('pageerror', e => { consoleLog.push({ type: 'pageerror', text: String(e) }); });
  page.on('requestfailed', r => consoleLog.push({ type: 'requestfailed', text: `${r.url()} ${r.failure()?.errorText}` }));

  await page.goto(url, { waitUntil: 'networkidle0', timeout: 60000 });

  const { scenarios } = await import(resolve(args.scenarios));
  for (const s of scenarios) {
    const before = consoleLog.length;
    try {
      await s.run(page, { url, out, root: resolve(args.root) });
      console.log(`PASS ${s.name}`);
    } catch (e) {
      failures++;
      console.log(`FAIL ${s.name}: ${e.message ?? e}`);
    }
    // A hung renderer must show up as that scenario's failure, not kill the
    // whole run before later scenarios and the console dump.
    try {
      await page.screenshot({ path: join(out, `${s.name}.png`), fullPage: true });
    } catch (e) {
      failures++;
      console.log(`FAIL ${s.name} (screenshot): ${String(e).slice(0, 140)}`);
    }
    const newErrors = consoleLog.slice(before).filter(m => m.type === 'error' || m.type === 'pageerror');
    if (newErrors.length) {
      failures++;
      console.log(`FAIL ${s.name} (console): ${newErrors.map(m => m.text).join(' | ').slice(0, 400)}`);
    }
  }
} finally {
  await writeFile(join(out, 'console.json'), JSON.stringify(consoleLog, null, 2));
  await browser.close();
  server.close();
}

console.log(failures ? `E2E FAILED: ${failures} failure(s)` : 'E2E OK');
process.exit(failures ? 1 : 0);
