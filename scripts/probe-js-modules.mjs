// Evaluate every browser JS asset's top level under a stub DOM.
//
//   node scripts/probe-js-modules.mjs
//
// `node --check` proves nothing here: an undefined reference is valid syntax.
// AGENTS.md records the shipped consequence, on 2026-08-07, when a scripted
// `replace()` matched nothing and left `PARAMS` referencing five constants
// that were never declared. `node --check` passed, `cargo check` passed, and
// the page died at module-eval time. ESM failure is fatal to the whole graph,
// so one file throwing meant another never defined its `window.*` functions
// and the page hung on a loading label with an unrelated-looking error.
//
// This is that check, made runnable and tracked rather than retyped from a
// note each time. It proves the top level executes. It does not exercise a
// single callback, which is a limit worth remembering: a modal handler broke
// in exactly that gap on this branch, and only a real endpoint payload run
// through the callback found it.
//
// In `scripts/` and not `notes/` on purpose: notes are gitignored, so
// anything an instruction file points at has to live somewhere tracked.

import { readdirSync } from 'node:fs';
import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const noop = () => {};
const el = () => ({
  style: {}, dataset: {}, classList: { add: noop, remove: noop, toggle: noop, contains: () => false },
  appendChild: noop, removeChild: noop, addEventListener: noop, removeEventListener: noop,
  setAttribute: noop, getAttribute: () => null, remove: noop, focus: noop, click: noop,
  querySelector: () => null, querySelectorAll: () => [], getBoundingClientRect: () => ({ width: 0, height: 0, top: 0, left: 0 }),
  insertAdjacentHTML: noop, scrollIntoView: noop, getContext: () => null,
});

globalThis.window = {
  addEventListener: noop, removeEventListener: noop, devicePixelRatio: 1,
  innerWidth: 1200, innerHeight: 800, location: { href: 'http://localhost/', search: '' },
  matchMedia: () => ({ matches: false, addEventListener: noop }),
  requestAnimationFrame: noop, cancelAnimationFrame: noop,
  setTimeout: noop, clearTimeout: noop, setInterval: noop, clearInterval: noop,
  getComputedStyle: () => ({ getPropertyValue: () => '' }),
};
globalThis.document = {
  addEventListener: noop, removeEventListener: noop, readyState: 'complete',
  hidden: false, fullscreenElement: null, fullscreenEnabled: false,
  webkitFullscreenElement: null, webkitFullscreenEnabled: false,
  createElement: el, createDocumentFragment: el, getElementById: () => null,
  querySelector: () => null, querySelectorAll: () => [], body: el(),
  documentElement: el(),
};
// `navigator` is a getter-only global in Node 22, so it is defined rather
// than assigned.
Object.defineProperty(globalThis, 'navigator', {
  value: { clipboard: { writeText: () => Promise.resolve() }, userAgent: 'probe' },
  configurable: true,
});
globalThis.localStorage = { getItem: () => null, setItem: noop, removeItem: noop };
globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
globalThis.EventSource = class { constructor() {} close() {} addEventListener() {} };
globalThis.AudioContext = class { constructor() {} createGain() { return { connect: noop, gain: {} }; } };
globalThis.echarts = {
  init: () => ({ setOption: noop, dispose: noop, resize: noop, on: noop,
                 getModel: () => null, getZr: () => ({ on: noop }) }),
  version: 'probe',
};

const dir = resolve('assets/js');
const files = readdirSync(dir).filter((f) => f.endsWith('.js')).sort();
let failed = 0;

for (const f of files) {
  try {
    await import(pathToFileURL(resolve(dir, f)).href);
    console.log(`ok    ${f}`);
  } catch (e) {
    failed++;
    console.log(`FAIL  ${f}: ${e.message}`);
  }
}

console.log(`\n${files.length - failed} of ${files.length} modules evaluated`);
process.exit(failed ? 1 : 0);
