// Behavioural probe for click-to-detail block selection.
//
//   node scripts/probe-click-detail.js assets/js/stats.js
//
// Exists because this logic is only reachable through a real DOM click on a
// live ECharts instance, so neither cargo test nor the endpoint probe can see
// it, and the failure is a wrong-but-plausible block height rather than an
// error. Run it after touching the click handler in stats.js.
//
// Sets up the exact disagreement: the cursor is inside block 965358's symbol
// (so ECharts hit-tests 965358 and the tooltip says 965358), while
// convertFromPixel returns a timestamp nearer 965359. The old value-space
// scan therefore picks 965359; the fix must pick 965358.
const fs = require('fs');
const listeners = {};
let echartsClick = null;
const opened = [];

const T58 = 1_700_000_000;   // block 965358
const T59 = 1_700_000_600;   // block 965359, ten minutes later

const fakeEl = {
  id: 'chart-x', _chart: null, _lazyVisible: true,
  isConnected: true, textContent: '', clientWidth: 1200, appendChild(){},
  addEventListener: (t, f) => { listeners[t] = f; },
  closest: () => null, getBoundingClientRect: () => ({ top: 0 }),
};
global.window = {
  innerWidth: 1400, scrollY: 0, scrollTo: () => {},
  location: { hash: '', href: 'http://x/' },
  addEventListener: () => {}, matchMedia: () => ({ matches: false }),
  showBlockDetail: (h) => opened.push(h),
};
global.document = {
  getElementById: () => fakeEl,
  createElement: () => ({ style: {}, setAttribute(){}, appendChild(){} }),
  addEventListener: () => {}, body: { appendChild(){} },
  querySelectorAll: () => [], querySelector: () => null,
};
global.IntersectionObserver = class { observe(){} disconnect(){} unobserve(){} };
global.ResizeObserver = class { observe(){} disconnect(){} };
global.requestAnimationFrame = (f) => f();
global.echarts = {
  init: () => ({
    on: (evt, fn) => { if (evt === 'click') echartsClick = fn; },
    setOption: () => {}, resize: () => {}, dispose: () => {},
    dispatchAction: () => {},
    getOption: () => ({ series: [{ data: [[T58, 9, 965358], [T59, 11, 965359]] }] }),
    containPixel: () => true,
    // Cursor pixel maps nearer to 965359 in value space.
    convertFromPixel: () => [T59 - 60, 0],
    getZr: () => ({ painter: {} }),
  }),
};

const origErr = console.error;
console.error = (...a) => origErr('SWALLOWED:', ...a);
eval(fs.readFileSync(process.argv[2], 'utf8'));
// stats.js defines its own showBlockDetail (which fetches). Replace it with
// the recorder AFTER eval so we observe the height it was called with.
global.window.showBlockDetail = (h) => opened.push(h);
global.window.initChart('chart-x');
global.window.setChartOption('chart-x', JSON.stringify({ series: [] }));

if (!listeners.mousedown || !listeners.mouseup) {
  console.log('FAIL: click listeners not registered'); process.exit(1);
}
console.log('listeners registered:', Object.keys(listeners).join(', '));
console.log('echarts click handler registered:', !!echartsClick);

function click({ onItem }) {
  opened.length = 0;
  listeners.mousedown({ offsetX: 100, offsetY: 100 });
  // ECharts hit-tests 965358. Pre-fix code registers no handler at all, which
  // is the bug: there is no way for the hit-test result to reach the modal.
  if (onItem && echartsClick) echartsClick({ data: [T58, 9, 965358] });
  listeners.mouseup({ offsetX: 100, offsetY: 100 });
  return new Promise(r => setTimeout(() => r(opened.slice()), 5));
}

(async () => {
  const onItem = await click({ onItem: true });
  console.log('\nclick landing ON block 965358 symbol -> opened', onItem,
              onItem[0] === 965358 ? ' PASS (matches tooltip)' : ' FAIL');

  const offItem = await click({ onItem: false });
  console.log('click on empty grid (no item hit)   -> opened', offItem,
              offItem[0] === 965359 ? ' PASS (value-space fallback still works)' : ' FAIL');

  const ok = onItem[0] === 965358 && offItem.length === 1 && offItem[0] === 965359;
  console.log('\n' + (ok ? 'BOTH PATHS CORRECT' : 'SOMETHING WRONG'));
  process.exit(ok ? 0 : 1);
})();
