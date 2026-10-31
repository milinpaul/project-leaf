// E14-S01: drives two tabs of the spike in headless Chromium, checks they converge, and records timings.
// Run: node drive.cjs <out-dir>   (serves web/ itself on port 8765)
const { chromium } = require("playwright");
const http = require("http"), fs = require("fs"), path = require("path");
const out = process.argv[2] || ".";
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = http.createServer((req, res) => {
  const file = path.join(__dirname, "web", req.url === "/" ? "index.html" : req.url.split("?")[0]);
  fs.readFile(file, (e, body) => { if (e) { res.writeHead(404); res.end(); return; }
    res.writeHead(200, { "content-type": types[path.extname(file)] || "application/octet-stream" }); res.end(body); });
}).listen(8765);

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 900, height: 640 } });
  const a = await context.newPage(), b = await context.newPage();
  const loadA = Date.now(); await a.goto("http://localhost:8765/"); await a.waitForFunction(() => window.__metrics?.loadMs > 0);
  const loadWall = Date.now() - loadA;
  await b.goto("http://localhost:8765/"); await b.waitForFunction(() => window.__metrics?.loadMs > 0);
  await a.waitForTimeout(300);

  // Tab A writes a small page with Markdown shortcuts, formatting, nesting and undo.
  await a.click("#page");
  await a.keyboard.type("# Trip to Kyoto");
  await a.keyboard.press("Enter");
  await a.keyboard.type("Things to pack, written in a browser.");
  await a.keyboard.press("Enter");
  await a.keyboard.type("- Passport");
  await a.keyboard.press("Enter");
  await a.keyboard.type("Rail pass");
  await a.keyboard.press("Enter");
  await a.keyboard.press("Tab");
  await a.keyboard.type("Reserve seats");
  await a.keyboard.press("Enter"); await a.keyboard.press("Shift+Tab");
  // Quick typing merges into one undo step, as on desktop: pause first so undo takes back only the typo.
  await a.waitForTimeout(1500);
  await a.keyboard.type("Typo here");
  await a.keyboard.press("Control+z");
  await a.keyboard.press("Backspace");
  await a.keyboard.press("Enter");
  await a.keyboard.type("[] Book the ryokan");
  await a.waitForTimeout(400);

  // Tab B, at the same time as A's last edits, adds a line of its own at the start.
  await b.click("#page");
  await b.keyboard.press("Home");
  await b.keyboard.press("Control+b");
  await b.keyboard.type("Shared with Sam: ");
  await b.keyboard.press("Control+b");
  await a.keyboard.type(" (2 nights)");
  await a.waitForTimeout(800);

  const textA = await a.evaluate(() => window.__spike()), textB = await b.evaluate(() => window.__spike());
  const mA = await a.evaluate(() => window.__metrics), mB = await b.evaluate(() => window.__metrics);
  await a.screenshot({ path: path.join(out, "web-spike-tab-a.png") });
  await b.screenshot({ path: path.join(out, "web-spike-tab-b.png") });
  await context.close();
  const dark = await browser.newContext({ viewport: { width: 900, height: 640 }, colorScheme: "dark" });
  const d = await dark.newPage(); await d.goto("http://localhost:8765/"); await d.waitForFunction(() => window.__metrics?.loadMs > 0);
  await d.click("#page"); await d.keyboard.type("# Dark mode"); await d.keyboard.press("Enter");
  await d.keyboard.type("The same view rows, drawn in the browser's dark theme.");
  await d.keyboard.press("Enter"); await d.keyboard.type("```"); await d.keyboard.press("Space");
  await d.keyboard.type("let leaf = \"in WebAssembly\";");
  await d.screenshot({ path: path.join(out, "web-spike-dark.png") });

  // A long page: keystroke time with 2,000 blocks.
  const big = await dark.newPage(); await big.goto("http://localhost:8765/"); await big.waitForFunction(() => window.__metrics?.loadMs > 0);
  await big.click("#page");
  for (let i = 0; i < 400; i++) { await big.keyboard.type(`Line ${i}`); await big.keyboard.press("Enter"); }
  const before = (await big.evaluate(() => window.__metrics.keyMs.length));
  for (let i = 0; i < 40; i++) await big.keyboard.type("x");
  const tail = await big.evaluate((n) => window.__metrics.keyMs.slice(n), before);
  const drawMs = await big.evaluate(() => { const s = performance.now(); document.getElementById("page").innerHTML = document.getElementById("page").innerHTML; return performance.now() - s; });
  await browser.close(); server.close();
  const median = (xs) => [...xs].sort((x, y) => x - y)[xs.length >> 1];
  console.log(JSON.stringify({
    converged: textA === textB, textA, textB,
    loadMs: { tabA: Math.round(mA.loadMs), tabB: Math.round(mB.loadMs), wall: loadWall },
    keyMsMedian: { small: +median(mA.keyMs).toFixed(2), page400Blocks: +median(tail).toFixed(2) },
    merges: { tabA: mA.merges, tabB: mB.merges }, redraw400BlocksMs: +drawMs.toFixed(1),
  }, null, 2));
})().catch((e) => { console.error(e); process.exit(1); });
