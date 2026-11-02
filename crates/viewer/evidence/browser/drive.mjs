// Usage: node drive.mjs CDP_PORT URL OUT_DIR WIDTH HEIGHT SCALE MOBILE STEPS
// STEPS: comma list of  wait:MS | click:X:Y | key:NAME | type:URITEXT | wheel:X:Y:DY | eval:URIJS | shot:NAME
const [,, port, url, outDir, w, h, scale, mobile, steps] = process.argv;
const sleep = ms => new Promise(r => setTimeout(r, ms));
let tabs;
for (let i = 0; i < 50; i++) { try { tabs = await (await fetch(`http://127.0.0.1:${port}/json`)).json(); break; } catch { await sleep(200); } }
const page = tabs.find(t => t.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise(r => ws.onopen = r);
let id = 0; const pend = new Map();
ws.onmessage = e => { const m = JSON.parse(e.data); if (m.id && pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); } };
const send = (method, params = {}) => new Promise(r => { const i = ++id; pend.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const fs = await import('fs');
const KEYS = { ArrowRight: 39, ArrowLeft: 37, Space: 32, '1': 49, '2': 50, '3': 51, Tab: 9, Enter: 13, ArrowUp: 38, ArrowDown: 40, t: 84, Backspace: 8 };
const CODES = { t: 'KeyT' };
await send('Emulation.setDeviceMetricsOverride', { width: +w, height: +h, deviceScaleFactor: +scale, mobile: mobile === 'true' });
await send('Page.navigate', { url });
for (const s of steps.split(',')) {
  const [op, a, b] = s.split(':');
  if (op === 'wait') await sleep(+a);
  else if (op === 'click') for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased']) await send('Input.dispatchMouseEvent', { type, x: +a, y: +b, button: 'left', clickCount: 1 });
  else if (op === 'key') { const code = CODES[a] || (/^\d$/.test(a) ? `Digit${a}` : a); for (const type of ['keyDown', 'keyUp']) await send('Input.dispatchKeyEvent', { type, key: a === 'Space' ? ' ' : a, code, windowsVirtualKeyCode: KEYS[a] }); }
  else if (op === 'type') for (const ch of decodeURIComponent(a)) { const up = ch.toUpperCase(); const code = /[a-z]/i.test(ch) ? `Key${up}` : /\d/.test(ch) ? `Digit${ch}` : ch === ' ' ? 'Space' : ''; for (const type of ['keyDown', 'keyUp']) await send('Input.dispatchKeyEvent', { type, key: ch, code, text: type === 'keyDown' ? ch : undefined, windowsVirtualKeyCode: up.charCodeAt(0) }); await sleep(60); }
  else if (op === 'wheel') { const [, x, y, dy] = s.split(':'); await send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: +x, y: +y, deltaX: 0, deltaY: +dy }); }
  else if (op === 'eval') { const r = await send('Runtime.evaluate', { expression: decodeURIComponent(a), returnByValue: true }); console.log('eval', JSON.stringify(r.result.result.value)); }
  else if (op === 'shot') { const r = await send('Page.captureScreenshot', { format: 'png' }); fs.writeFileSync(`${outDir}/${a}.png`, Buffer.from(r.result.data, 'base64')); console.log('saved', a); }
}
ws.close(); process.exit(0);
