// E14-S01 spike: the browser forwards input to leaf-editor (WebAssembly) and redraws from its view rows. Throwaway.
import init, { Spike } from "./pkg/leaf_web_spike.js";

const t0 = performance.now();
await init();
const page = document.getElementById("page");
const input = document.getElementById("input");
const metrics = { loadMs: 0, keyMs: [], merges: 0 };
window.__metrics = metrics;

// Copies of the page in other tabs talk over a BroadcastChannel: no server.
const channel = new BroadcastChannel("leaf-spike");
const me = Math.random().toString(36).slice(2, 6);
document.getElementById("who").textContent = `tab ${me}`;
// A new tab asks the others for the page and opens their copy; only the first tab creates one. Two pages created
// on their own would merge into one with both first lines, so a page is created once and then shared.
let spike = new Spike();
let fresh = true;
channel.postMessage({ from: me, hello: true });

function draw() { page.innerHTML = spike.html(); }
function status(id, text) { document.getElementById(id).textContent = text; }

let sendTimer = null;
function changed() {
  fresh = false;
  draw();
  clearTimeout(sendTimer);
  sendTimer = setTimeout(() => channel.postMessage({ from: me, snapshot: spike.snapshot() }), 50);
}
function timed(f) {
  const s = performance.now();
  const used = f();
  const ms = performance.now() - s;
  metrics.keyMs.push(ms);
  const sorted = [...metrics.keyMs].sort((a, b) => a - b);
  status("key", `keystroke ${ms.toFixed(1)} ms (median ${sorted[sorted.length >> 1].toFixed(1)} ms)`);
  return used;
}

channel.onmessage = ({ data }) => {
  if (data.from === me) return;
  if (data.hello) { channel.postMessage({ from: me, snapshot: spike.snapshot() }); return; }
  if (data.snapshot && fresh) {
    spike = Spike.from_snapshot(data.snapshot);
    fresh = false;
    draw();
    return;
  }
  if (data.snapshot) {
    spike.merge(data.snapshot);
    metrics.merges++;
    status("sync", `merged ${metrics.merges} from other tabs`);
    draw();
  }
};

// Typing goes through a hidden textarea, so input methods and the emoji picker work; never contenteditable.
let composing = false;
input.addEventListener("compositionstart", () => { composing = true; });
input.addEventListener("compositionend", (e) => { composing = false; timed(() => spike.type_text(e.data)); input.value = ""; changed(); });
input.addEventListener("input", (e) => {
  if (composing || e.isComposing) return;
  if (input.value) { const text = input.value; input.value = ""; timed(() => spike.type_text(text)); changed(); }
});
input.addEventListener("keydown", (e) => {
  if (composing || e.isComposing) return;
  const command = e.metaKey || e.ctrlKey;
  if (e.key.length === 1 && !command) return; // a character: the input event types it
  if (timed(() => spike.key(e.key, e.shiftKey, command))) { e.preventDefault(); changed(); }
});
page.addEventListener("mousedown", (e) => { e.preventDefault(); input.focus(); });
input.addEventListener("focus", () => page.classList.add("focused"));
input.addEventListener("blur", () => page.classList.remove("focused"));

draw();
input.focus();
metrics.loadMs = performance.now() - t0;
status("load", `loaded in ${metrics.loadMs.toFixed(0)} ms`);
window.__spike = () => spike.text();
