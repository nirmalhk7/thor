// Independent web-surface check; does not substitute for native CEF verification.
// Requires Playwright on NODE_PATH and Chromium (or Playwright's installed browser).
// Run: node scripts/check_workbench_layout.cjs
// Optional: THOR_CHROMIUM_EXECUTABLE, THOR_SCREENSHOT_DIR.
const assert = require('node:assert/strict');
const http = require('node:http');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const root = path.join(__dirname, '../src/workbench');
const routes = { '/': 'home.html', '/api': 'api.html', '/security': 'security.html', '/tools': 'tools.html' };
const server = http.createServer((req, res) => {
  const route = new URL(req.url, 'http://localhost').pathname;
  const name = routes[route] || route.slice(1);
  if (!/^[\w.-]+$/.test(name)) { res.writeHead(404).end(); return; }
  const file = path.join(root, name);
  if (!fs.existsSync(file)) { res.writeHead(404).end(); return; }
  res.setHeader('Content-Type', name.endsWith('.css') ? 'text/css' : name.endsWith('.js') ? 'text/javascript' : 'text/html');
  res.end(fs.readFileSync(file));
});

async function main() {
  const output = process.env.THOR_SCREENSHOT_DIR;
  if (output) fs.mkdirSync(output, { recursive: true });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  let browser;
  try {
    browser = await chromium.launch({
      executablePath: process.env.THOR_CHROMIUM_EXECUTABLE || undefined,
      headless: true,
    });
    for (const width of [712, 281]) {
      const page = await browser.newPage({ viewport: { width, height: width === 712 ? 415 : 380 } });
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      for (const route of Object.keys(routes)) {
        await page.goto(`http://127.0.0.1:${server.address().port}${route}`);
        await page.evaluate(() => document.fonts.ready);
        assert.equal(await page.evaluate(() => document.documentElement.scrollWidth), width, `${route}: horizontal overflow at ${width}px`);
        if (output) await page.screenshot({ path: path.join(output, `${route === '/' ? 'home' : route.slice(1)}-${width}.png`), fullPage: true });
        if (route === '/api') {
          await page.locator('#tab-headers').click();
          assert.equal(await page.locator('#tab-headers').getAttribute('aria-selected'), 'true');
          assert.equal(await page.locator('#panel-headers').isVisible(), true);
          await page.locator('#tab-auth').click();
          await page.locator('#auth-type').selectOption('bearer');
          assert.equal(await page.locator('#token').isEnabled(), true);
          await page.locator('#auth-type').selectOption('none');
          assert.equal(await page.locator('#token').isDisabled(), true);
        }
        assert.deepEqual(errors, [], `${route}: unhandled browser errors`);
        console.log(`PASS ${route} at ${width}px`);
      }
      await page.close();
    }
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
