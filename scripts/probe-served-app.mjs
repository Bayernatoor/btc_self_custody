// Drive the served application over CDP and read back what it rendered.
//
//   BASE=http://127.0.0.1:8011 node scripts/probe-served-app.mjs
//
// Chrome must already be listening on --remote-debugging-port=9222. The
// wrapper that launches it, drives this, and kills it lives in
// `scripts/probe-served-app.sh`, because a backgrounded browser does not
// survive the tool call that started it in an agent session.
//
// # Why this exists
//
// Every check before this one was at the option level: options dumped from
// the Rust builders and rendered in a bare page. That proves the builders and
// it proves ECharts accepts their output. It says nothing about the app.
// Nothing had loaded a route, waited for hydration, or watched a chart appear.
//
// What this adds, per page: the console is watched for errors, every canvas is
// measured, and every live ECharts instance is interrogated for its series and
// point counts. A chart with a canvas and no series is the failure that looks
// fine in a screenshot.
//
// # What a chart being here at all proves
//
// The server renders the page shell; the charts are built in WASM and handed
// to ECharts through `wasm_bindgen` externs. So a populated ECharts instance
// is evidence that the WASM module loaded, hydration ran, the resource
// resolved, and the JS bridge worked. That is the hydration check the
// acceptance matrix said nothing covered.

const BASE = process.env.BASE || 'http://127.0.0.1:8011';
const DEBUG = process.env.CDP || 'http://127.0.0.1:9222';

const PAGES = [
  ['/observatory/charts/network', 'Network category page'],
  ['/observatory/charts/fees', 'Fees category page'],
  ['/observatory/charts/mining', 'Mining category page'],
  ['/observatory/charts/embedded', 'Embedded category page'],
  ['/observatory/chart/diff-adjustment', 'single chart, retarget-sourced'],
  ['/observatory/chart/diff-adjustment?range=1d', 'single chart, no retarget in range'],
  ['/observatory/chart/interval', 'single chart, daily rate'],
  ['/observatory/chart/diversity', 'single chart, gauge'],
  ['/observatory/chart/batching', 'single chart, several measurements'],
  ['/observatory/chart/fee-heatmap?range=1y', 'single chart, no daily builder at a long range'],
];

// Read once per page, inside the page, after the charts have had time to draw.
const COLLECT = `(() => {
  const canvases = [...document.querySelectorAll('canvas')].map(c => ({
    w: c.width, h: c.height,
  }));
  const charts = [];
  if (window.echarts) {
    document.querySelectorAll('div').forEach(d => {
      const inst = window.echarts.getInstanceByDom(d);
      if (!inst) return;
      let series = [];
      try {
        const model = inst.getModel();
        series = model.getSeries().map(s => {
          const data = s.getData();
          const t = s.get('type');
          const dim = (t === 'pie' || t === 'gauge')
            ? data.mapDimension('value') : data.mapDimension('y');
          let pts = 0;
          data.each(i => {
            const v = data.get(dim, i);
            if (v !== null && v !== undefined && !(typeof v === 'number' && isNaN(v))) pts++;
          });
          return { name: s.name, type: t, points: pts };
        });
      } catch (e) { series = [{ name: 'THREW: ' + e.message, type: '?', points: 0 }]; }
      charts.push({ id: d.id || '(anonymous)', series });
    });
  }
  const text = document.body.innerText || '';
  return {
    canvases,
    charts,
    hasEcharts: !!window.echarts,
    // Phrases the acceptance matrix asks a human to look for.
    saysNoAdjustment: text.includes('No difficulty adjustment in this range'),
    saysShorterRange: text.includes('shorter range'),
    saysNotAvailable: text.includes('Not available for this range'),
    saysNoSummary: text.includes('A single average, peak or change is not meaningful'),
    saysSeveralMeasurements: text.includes('several separate measurements'),
    saysLoading: text.includes('Loading chart data'),
    saysMining: text.includes('Mining blocks'),
    definitionShown: text.includes('Definition'),
    kpiLabels: ['average','peak','low','change','observations']
      .filter(l => text.includes(l)),
  };
})()`;

async function cdp() {
  // The page target, not the browser one. `/json/version` hands back the
  // browser-level socket, which carries Target and Browser domains and
  // answers "'Page.enable' wasn't found" to everything useful.
  const list = await (await fetch(DEBUG + '/json/list')).json();
  const page = list.find((t) => t.type === 'page' && t.webSocketDebuggerUrl);
  if (!page) throw new Error('no page target on ' + DEBUG);
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok, no) => { ws.onopen = ok; ws.onerror = no; });
  let id = 0;
  const pending = new Map();
  const events = [];
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && pending.has(msg.id)) {
      const { ok, no } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? no(new Error(msg.error.message)) : ok(msg.result);
    } else if (msg.method) {
      events.push(msg);
    }
  };
  const send = (method, params = {}) => new Promise((ok, no) => {
    const n = ++id;
    pending.set(n, { ok, no });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
  return { send, events, close: () => ws.close() };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const { send, events, close } = await cdp();
await send('Page.enable');
await send('Runtime.enable');
await send('Log.enable');

let failures = 0;
const report = [];

for (const [path, label] of PAGES) {
  events.length = 0;
  await send('Page.navigate', { url: BASE + path });
  // Hydration plus a resource round trip plus the chart draw. Generous
  // because a cold cache on the ALL range is genuinely slow.
  await sleep(9000);
  const { result } = await send('Runtime.evaluate', {
    expression: COLLECT, returnByValue: true, awaitPromise: false,
  });
  const r = result.value || {};

  const consoleErrors = events
    .filter((e) => e.method === 'Runtime.exceptionThrown'
      || (e.method === 'Runtime.consoleAPICalled' && e.params.type === 'error')
      || (e.method === 'Log.entryAdded' && e.params.entry.level === 'error'))
    .map((e) => e.method === 'Runtime.exceptionThrown'
      ? (e.params.exceptionDetails.exception?.description
         || e.params.exceptionDetails.text)
      : (e.params.entry?.text
         || (e.params.args || []).map((a) => a.value ?? a.description).join(' ')))
    // The favicon and any /api 4xx a page legitimately probes are noise.
    .filter((t) => t && !/favicon/i.test(t));

  const populated = (r.charts || []).filter(
    (c) => c.series.some((s) => s.points > 0));
  const empty = (r.charts || []).filter(
    (c) => !c.series.some((s) => s.points > 0));

  const lines = [];
  const fail = (m) => { lines.push('  FAIL ' + m); failures++; };

  if (!r.hasEcharts) fail('ECharts never loaded on the page');
  if (consoleErrors.length) fail('console errors: '
    + JSON.stringify(consoleErrors.slice(0, 3)));

  // A category page must draw something, or hydration did not complete.
  if (path.startsWith('/observatory/charts/') && populated.length === 0) {
    fail('no chart on the page resolved a single point, so either hydration '
      + 'did not run or every resource failed');
  }

  // Page-specific expectations from the acceptance matrix.
  if (path.includes('diff-adjustment?range=1d') && !r.saysNoAdjustment) {
    fail('a 1D range holds no retarget, so the chart should say so');
  }
  if (path.includes('chart/diversity')) {
    if (r.saysSeveralMeasurements) {
      fail('the gauge rail claims several separate measurements');
    }
    if (!r.saysNoSummary) {
      fail('the gauge rail should say a single summary is not meaningful');
    }
  }
  if (r.saysMining) fail('the old "Mining blocks" loading label is still shown');

  report.push([
    (lines.length ? 'FAIL ' : 'ok   ') + path,
    `       ${label}`,
    `       charts ${(r.charts || []).length} populated ${populated.length}`
      + ` empty ${empty.length} canvases ${(r.canvases || []).length}`,
    ...(empty.length ? [`       empty: ${empty.map((c) => c.id).join(', ')}`] : []),
    `       kpi labels: ${(r.kpiLabels || []).join(',') || 'none'}`
      + (r.saysNotAvailable ? ' | says not-available' : '')
      + (r.saysNoSummary ? ' | says no-summary' : '')
      + (r.saysShorterRange ? ' | says shorter-range' : ''),
    ...lines,
  ].join('\n'));
}

close();
console.log(report.join('\n\n'));
console.log(`\n${PAGES.length} pages driven, ${failures} failures`);
process.exit(failures ? 1 : 0);
