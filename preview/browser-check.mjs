import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

const root = new URL('.', import.meta.url);
const elements = new Map();
const sockets = [];
let rejectOldFetch;
class FakeElement {
  children = [];
  classes = new Set();
  classList = {
    add: (...names) => names.forEach((name) => this.classes.add(name)),
    remove: (...names) => names.forEach((name) => this.classes.delete(name)),
  };
  hidden = false;
  textContent = '';
  removeAttribute() {}
  querySelector() {
    return this.children[0] ?? null;
  }
  replaceChildren(...kids) {
    this.children = kids;
  }
  appendChild(kid) {
    this.children.push(kid);
    return kid;
  }
  remove() {}
}
// The crossfading status line keeps the current text in its newest child span.
const statusText = () => {
  const el = elements.get('status');
  return el.children.length ? el.children.at(-1).textContent : el.textContent;
};
class Socket {
  static OPEN = 1;
  readyState = 1;
  constructor() {
    sockets.push(this);
  }
  close() {
    this.readyState = 3;
    this.onclose();
  }
}
await runInNewContext(await readFile(new URL('app.js', root), 'utf8'), {
  URL,
  document: {
    querySelector: () => ({ content: 'https://preview.example.test' }),
    getElementById: (id) => {
      if (!elements.has(id)) elements.set(id, new FakeElement());
      return elements.get(id);
    },
    createElement: () => new FakeElement(),
  },
  Terminal: class {
    parser = { registerOscHandler() {} };
    loadAddon() {}
    open() {}
    onData() {}
    onResize() {}
    reset() {}
  },
  FitAddon: {
    FitAddon: class {
      fit() {}
    },
  },
  ResizeObserver: class {
    observe() {}
  },
  WebSocket: Socket,
  window: { addEventListener() {} },
  matchMedia: () => ({ matches: false }),
  setTimeout: () => 0,
  clearTimeout: () => {},
  fetch: () =>
    sockets.length === 1
      ? new Promise((_, reject) => (rejectOldFetch = reject))
      : Promise.resolve({
          ok: true,
          json: async () => ({ generated_at: '2026-01-01T00:00:00.000Z' }),
        }),
});
const readyEvent = { data: JSON.stringify({ type: 'ready', hash: 'a'.repeat(64) }) };
const retiredFetch = sockets[0].onmessage(readyEvent);
sockets[0].close();
elements.get('restart').onclick();
await sockets[1].onmessage(readyEvent);
assert.equal(statusText(), 'Ready');
rejectOldFetch(Error('Old session snapshot failed'));
await retiredFetch;
assert.equal(statusText(), 'Ready');
assert.equal(sockets[1].readyState, Socket.OPEN);
assert.equal(elements.get('restart').hidden, true);
console.log('PASS Retired snapshot fetch failure preserves ready replacement status');
