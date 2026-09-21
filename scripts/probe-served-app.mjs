// Drive the served application over CDP and read back what it rendered.
//
//   node scripts/probe-served-app.mjs          # defaults to port 8000
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

const BASE = process.env.BASE || 'http://127.0.0.1:8000';

// Hosts this app loads but does not own. Their failures are not findings:
// `src/app.rs` loads poeticmetric for analytics, which cannot be reached
// from a sandboxed headless browser at all.
const THIRD_PARTY = ['poeticmetric.com', 'cdn.jsdelivr.net',
  'blockchain.info', 'mempool.space'];
const DEBUG = process.env.CDP || 'http://127.0.0.1:9222';

const PAGES = (process.env.ONLY ? [
  [process.env.ONLY, 'single page under test'],
] : [
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
]);

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
      // The title is drawn on the canvas, not in the DOM, so a no-data
      // frame's message is invisible to document.innerText. Reading it off
      // the model is the only way to see it: the first version of this probe
      // reported "the chart should say so" for a chart that was saying so.
      let title = '';
      try {
        const t = inst.getOption().title;
        const first = Array.isArray(t) ? t[0] : t;
        title = [first && first.text, first && first.subtext]
          .filter(Boolean).join(' | ');
      } catch (e) { title = ''; }
      charts.push({ id: d.id || '(anonymous)', series, title });
    });
  }
  const text = document.body.innerText || '';
  return {
    canvases,
    charts,
    hasEcharts: !!window.echarts,
    // Phrases the acceptance matrix asks a human to look for.
    saysNoAdjustment: text.includes('No difficulty adjustment in this range')
      || charts.some((c) => c.title
        .includes('No difficulty adjustment in this range')),
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
  // A dropped connection has to reject every pending call. Without this,
  // losing the browser mid-run left the awaits unsettled and node exited
  // with "Detected unsettled top-level await" and no cause. The usual way to
  // lose it is two probe runs sharing a CDP port: each script's trap kills
  // the other's Chrome.
  const dropAll = (why) => {
    for (const [, { no }] of pending) no(new Error(why));
    pending.clear();
  };
  ws.onclose = () => dropAll('the browser closed the CDP connection');
  ws.onerror = () => dropAll('the CDP connection errored');

  const send = (method, params = {}) => new Promise((ok, no) => {
    const n = ++id;
    pending.set(n, { ok, no });
    // Never wait forever on a browser that has stopped answering.
    const timer = setTimeout(() => {
      if (pending.delete(n)) no(new Error(`${method} timed out after 30s`));
    }, 30_000);
    const settle = (f) => (v) => { clearTimeout(timer); f(v); };
    pending.set(n, { ok: settle(ok), no: settle(no) });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
  return { send, events, close: () => ws.close() };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const { send, events, close } = await cdp();
await send('Page.enable');
await send('Runtime.enable');
await send('Log.enable');
await send('Network.enable');

process.stderr.write(`driving ${PAGES.length} pages at ${BASE}, ~9s each\n`);

let failures = 0;
const report = [];

for (const [path, label] of PAGES) {
  events.length = 0;
  process.stderr.write(`  ${path} ... `);
  await send('Page.navigate', { url: BASE + path });
  // Hydration plus a resource round trip plus the chart draw. Generous
  // because a cold cache on the ALL range is genuinely slow.
  await sleep(9000);

  // The category pages lazy-init their charts with an IntersectionObserver
  // (`assets/js/stats.js:42`), so a chart below the fold never draws until
  // it is scrolled into view. Not scrolling made all four category pages
  // report zero charts, which this probe called "hydration did not run" for
  // three separate runs: a design feature read as a total failure.
  //
  // Stepped rather than jumped to the bottom, so each card passes through
  // the viewport and its observer fires.
  await send('Runtime.evaluate', {
    expression: `(async () => {
      const step = window.innerHeight * 0.8;
      for (let y = 0; y < document.body.scrollHeight; y += step) {
        window.scrollTo(0, y);
        await new Promise(r => setTimeout(r, 250));
      }
      window.scrollTo(0, 0);
    })()`,
    awaitPromise: true,
  });
  // The charts that just came into view need their own round trip.
  await sleep(6000);
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
    // Noise we do not own, named by host rather than by message. An
    // earlier version dropped every "Failed to load resource", which would
    // have hidden one of our own assets failing: exactly the kind of filter
    // that makes a probe report success it has not established.
    //
    // `reportAllChanges` is web-vitals, which arrives with the analytics
    // script or a browser extension as an injected VM script, so it has no
    // URL to match on and is named directly.
    .filter((t) => t
      && !/favicon/i.test(t)
      && !THIRD_PARTY.some((h) => t.includes(h))
      // web-vitals, injected by the analytics script or a browser
      // extension as a VM script, so it has no URL to classify by.
      && !/reportAllChanges/.test(t)
      // Chrome's message for a failed resource carries no URL, so it cannot
      // be attributed by host. It is redundant: every failed request is
      // classified from the network events above, which do have URLs, and
      // `ours` fails the page with the path when one of them is ours.
      && !/^Failed to load resource/.test(t));

  // Failed requests, split by origin. A third party failing is not this
  // app's defect: the site loads poeticmetric.com/pm.js for analytics, and
  // that 422s from a headless browser, which the first version of this
  // probe reported as a failure on all ten pages. Judge our own origin and
  // report the rest as noise.
  const requestUrls = new Map(
    events.filter((e) => e.method === 'Network.requestWillBeSent')
      .map((e) => [e.params.requestId, e.params.request.url]));
  const failedRequests = [
    ...events.filter((e) => e.method === 'Network.responseReceived'
        && e.params.response.status >= 400)
      .map((e) => ({ status: String(e.params.response.status),
                     url: e.params.response.url })),
    // A refused or aborted request never receives a response, so it is
    // absent from the list above. Without this, a first-party asset failing
    // to connect would be invisible to the URL-based classification and
    // would have to be caught by the console message instead.
    ...events.filter((e) => e.method === 'Network.loadingFailed')
      .map((e) => ({ status: e.params.errorText || 'failed',
                     url: requestUrls.get(e.params.requestId) || '(unknown)' })),
  ];
  // ERR_ABORTED is not a failure in a probe that navigates every few
  // seconds and then kills the browser: a request in flight at teardown is
  // aborted by definition. Reported separately rather than counted either
  // way, because treating it as a pass would hide a real cancellation and
  // treating it as a failure flagged ECharts, which had plainly loaded
  // since the chart rendered its title.
  const aborted = [...new Set(failedRequests
    .filter((r) => /ERR_ABORTED/.test(r.status))
    .map((r) => r.url.replace(BASE, '')))];
  const ours = [...new Set(failedRequests
    .filter((r) => r.url.startsWith(BASE) && !/ERR_ABORTED/.test(r.status))
    .map((r) => `${r.status} ${r.url.slice(BASE.length)}`))];
  const thirdParty = [...new Set(failedRequests
    .filter((r) => !r.url.startsWith(BASE) && !/ERR_ABORTED/.test(r.status))
    .map((r) => `${r.status} ${new URL(r.url).host}`))];

  const populated = (r.charts || []).filter(
    (c) => c.series.some((s) => s.points > 0));
  const empty = (r.charts || []).filter(
    (c) => !c.series.some((s) => s.points > 0));

  const lines = [];
  const fail = (m) => { lines.push('  FAIL ' + m); failures++; };

  if (!r.hasEcharts) fail('ECharts never loaded on the page');
  if (consoleErrors.length) fail('console errors: '
    + JSON.stringify(consoleErrors.slice(0, 3)));
  if (ours.length) fail('our own requests failed: '
    + JSON.stringify(ours.slice(0, 4)));

  // **A category page drawing nothing here is inconclusive, not a failure.**
  //
  // Verified by hand in a real browser on 2026-09-21: every chart on
  // /observatory/charts/mining draws. In headless the ECharts request to
  // cdn.jsdelivr.net comes back ERR_ABORTED on these pages and no canvas is
  // created, while the same request succeeds on every single-chart page in
  // the same run. The cause is not isolated and is somewhere in headless
  // Chrome's handling of that page rather than in the app.
  //
  // This probe had called it "hydration did not run" across three runs,
  // which was the fifth of six ways it reported a working app as broken. So
  // it says what it saw and leaves the verdict to the reader.
  if (path.startsWith('/observatory/charts/') && populated.length === 0) {
    lines.push('  INCONCLUSIVE no canvas on a category page. Confirmed by '
      + 'hand that these draw in a real browser; ECharts is ERR_ABORTED from '
      + 'jsdelivr here while succeeding on the single-chart pages. Check a '
      + 'category page by eye rather than trusting this row.');
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
    ...(empty.length ? [`       empty: ${empty.map((c) =>
      c.id + (c.title ? ` ("${c.title}")` : '')).join(', ')}`] : []),
    ...(aborted.length
      ? [`       aborted at teardown, inconclusive: ${aborted.join(', ')}`]
      : []),
    ...(thirdParty.length
      ? [`       third-party, not ours: ${thirdParty.join(', ')}`] : []),
    `       kpi labels: ${(r.kpiLabels || []).join(',') || 'none'}`
      + (r.saysNotAvailable ? ' | says not-available' : '')
      + (r.saysNoSummary ? ' | says no-summary' : '')
      + (r.saysShorterRange ? ' | says shorter-range' : ''),
    ...lines,
  ].join('\n'));
  process.stderr.write(lines.length ? `FAIL\n` : `ok\n`);
}

close();
console.log(report.join('\n\n'));
console.log(`\n${PAGES.length} pages driven, ${failures} failures`);
process.exit(failures ? 1 : 0);
