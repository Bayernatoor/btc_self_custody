// Prove the Heartbeat notable-tx feed cannot be made to execute script by the
// contents of an OP_RETURN.
//
//   node scripts/test-heartbeat-feed-escaping.mjs
//
// Anyone who can pay a transaction fee chooses the bytes in `op_return_text`,
// and `notable.rs::extract_readable_text` admits anything 70% printable. So
// this is not a theoretical input: it is a field an outsider writes, rendered
// on the most-visited page on the site.
//
// Two independent assertions, because either alone is weak:
//
//   1. `innerHTML` is a THROWING SETTER in the shim below. Any code path that
//      builds a row from a markup string fails loudly rather than subtly. This
//      is the structural guarantee, and it holds for payloads nobody imagined.
//   2. The payload must come back as TEXT, byte for byte, in both the visible
//      cell and the title. A fix that escaped on the way in would store
//      `&lt;img&gt;` and fail this, which is correct: the feed should display
//      what was broadcast, not an escaped rendering of it.
//
// `node --check` is not a substitute. It validates syntax, and an undefined
// reference is valid syntax; this project shipped a dead page that way on
// 2026-08-07. This harness evaluates the module.

import { pathToFileURL } from 'node:url';

const MODULE = process.argv[2] || './assets/js/heartbeat-sse.js';

// ── Minimal DOM ────────────────────────────────────────────────────────────
let innerHTMLAttempts = 0;

function makeEl(tag) {
    const el = {
        tagName: tag,
        className: '',
        title: '',
        _text: '',
        dataset: {},
        style: {},
        children: [],
        classList: { add() {}, remove() {}, contains: () => false },
        appendChild(c) { this.children.push(c); return c; },
        insertBefore(c) { this.children.unshift(c); return c; },
        removeChild(c) {
            const i = this.children.indexOf(c);
            if (i >= 0) this.children.splice(i, 1);
            return c;
        },
        remove() {},
        querySelector(sel) {
            if (sel === '[data-placeholder]') return null;
            const want = sel.replace(/^\[|\]$/g, '');
            const walk = (n) => {
                for (const c of n.children) {
                    if (want === 'data-txid-link' && c.dataset.txidLink != null) return c;
                    const hit = walk(c);
                    if (hit) return hit;
                }
                return null;
            };
            return walk(this);
        },
        addEventListener() {},
        setAttribute(k, v) { if (k === 'title') this.title = v; },
    };
    Object.defineProperty(el, 'textContent', {
        get() { return this._text; },
        set(v) { this._text = String(v); },
    });
    // The structural guarantee. Assigning markup is the bug, so make it fatal.
    Object.defineProperty(el, 'innerHTML', {
        get() { return ''; },
        set() {
            innerHTMLAttempts++;
            throw new Error('innerHTML assigned: the feed row is being built from a markup string');
        },
    });
    Object.defineProperty(el, 'firstChild', { get() { return this.children[0] || null; } });
    Object.defineProperty(el, 'lastChild', {
        get() { return this.children[this.children.length - 1] || null; },
    });
    return el;
}

const panel = makeEl('div');
const list = makeEl('div');

globalThis.document = {
    createElement: makeEl,
    getElementById: (id) =>
        id === 'whale-feed-panel' ? panel : id === 'whale-feed-list' ? list : null,
    querySelectorAll: () => [],
    addEventListener() {},
};
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem: () => {} };
globalThis.requestAnimationFrame = () => 0;
globalThis.EventSource = function () { return { addEventListener() {}, close() {} }; };

// ── The payloads ───────────────────────────────────────────────────────────
// Each is a real escape against string-concatenated HTML. The second and third
// break out of the `title="..."` attribute, which the old code "escaped" by
// replacing double quotes only.
const PAYLOADS = [
    '<img src=x onerror=alert(1)>',
    '" onmouseover="alert(1)',
    "' onfocus='alert(1)",
    '<script>alert(1)</scr' + 'ipt>',
    '</span><svg/onload=alert(1)>',
    '&lt;already escaped&gt;',
];

function textOf(node) {
    let s = node._text || '';
    for (const c of node.children) s += textOf(c);
    return s;
}

function titlesOf(node, acc = []) {
    if (node.title) acc.push(node.title);
    for (const c of node.children) titlesOf(c, acc);
    return acc;
}

const failures = [];

const mod = await import(pathToFileURL(MODULE).href);
if (typeof window._notableFeed !== 'function') {
    console.error('FAIL: module did not define window._notableFeed');
    process.exit(1);
}

for (let i = 0; i < PAYLOADS.length; i++) {
    const payload = PAYLOADS[i];
    list.children.length = 0;
    const tx = {
        txid: 'ff'.repeat(32) + i,
        op_return_msg: true,
        op_return_text: payload,
        value: 12345678,
        fee: 500,
        vsize: 200,
        first_seen: 1700000000,
    };

    let threw = null;
    try {
        window._notableFeed(tx);
    } catch (e) {
        threw = e;
    }

    if (threw) {
        failures.push(`payload ${i}: ${threw.message}`);
        continue;
    }

    const row = list.children[0];
    if (!row) { failures.push(`payload ${i}: no row rendered`); continue; }

    // Assertion 2a: the visible cell holds the payload's first 30 chars as text.
    const expectVisible = payload.substring(0, 30);
    if (!textOf(row).includes(expectVisible)) {
        failures.push(
            `payload ${i}: visible text missing the literal payload\n` +
            `  want substring: ${JSON.stringify(expectVisible)}\n` +
            `  got text:       ${JSON.stringify(textOf(row))}`);
    }

    // Assertion 2b: the title holds the WHOLE payload, unescaped and unmangled.
    if (!titlesOf(row).includes(payload)) {
        failures.push(
            `payload ${i}: title does not carry the payload verbatim\n` +
            `  want: ${JSON.stringify(payload)}\n` +
            `  got:  ${JSON.stringify(titlesOf(row))}`);
    }
}

// ── Every branch still renders ─────────────────────────────────────────────
// The escaping fix rewrote all eight label branches, so a typo in any of them
// would ship a blank or wrong badge while the payload tests above stayed green.
// The class string is checked as well as the label, because rebuilding these
// from markup meant retyping sixteen Tailwind strings by hand. A typo there
// renders an unstyled or wrong-coloured badge, and every other assertion in
// this file would stay green while it happened.
const BRANCHES = [
    ['whale',         { whale: true, value: 500000000, value_usd: 250000 }, 'WHALE',       'text-[#ffd700] font-bold shrink-0'],
    ['round_number',  { round_number: true, max_output_value: 100000000 },  'ROUND #',     'text-[#90ee90] font-bold shrink-0'],
    ['inscription',   { large_inscription: true, value: 5000 },             'INSCRIPTION', 'text-[#ff00c8] font-bold shrink-0'],
    ['consolidation', { consolidation: true, input_count: 60, output_count: 1 }, 'CONSOL',  'text-[#a855f7] font-bold shrink-0'],
    ['fan_out',       { fan_out: true, input_count: 1, output_count: 40 },  'FAN-OUT',     'text-[#00d2ff] font-bold shrink-0'],
    ['fee_outlier',   { fee_outlier: true, fee: 50000, vsize: 200 },        'FEE',         'text-[#ff4444] font-bold shrink-0'],
    ['other',         {},                                                   'NOTABLE',     'text-white/50 font-bold shrink-0'],
];

// Cells every row carries regardless of branch.
const ALWAYS_CLASSES = [
    'text-white/30 hover:text-[#f7931a] transition-colors cursor-pointer',  // txid
    'text-white/20 ml-auto shrink-0',                                       // time
];

function classesOf(node, acc = []) {
    if (node.className) acc.push(node.className);
    for (const c of node.children) classesOf(c, acc);
    return acc;
}

for (let i = 0; i < BRANCHES.length; i++) {
    const [name, flags, wantLabel, wantClass] = BRANCHES[i];
    list.children.length = 0;
    const tx = Object.assign(
        { txid: 'ab'.repeat(32) + i, value: 1000, fee: 100, vsize: 100, first_seen: 1700000000 },
        flags);
    try {
        window._notableFeed(tx);
    } catch (e) {
        failures.push(`branch ${name}: threw ${e.message}`);
        continue;
    }
    const row = list.children[0];
    if (!row) { failures.push(`branch ${name}: no row rendered`); continue; }
    if (!textOf(row).includes(wantLabel)) {
        failures.push(
            `branch ${name}: label missing\n` +
            `  want: ${JSON.stringify(wantLabel)}\n` +
            `  got:  ${JSON.stringify(textOf(row))}`);
    }
    // Every row must carry its txid link and its type, or the filter breaks.
    if (row.dataset.type !== (name === 'fee_outlier' ? 'fee' : name)) {
        failures.push(`branch ${name}: dataset.type is ${JSON.stringify(row.dataset.type)}`);
    }
    if (!row.querySelector('[data-txid-link]')) {
        failures.push(`branch ${name}: no clickable txid cell`);
    }
    const cls = classesOf(row);
    if (!cls.includes(wantClass)) {
        failures.push(
            `branch ${name}: label class wrong\n` +
            `  want: ${JSON.stringify(wantClass)}\n` +
            `  got:  ${JSON.stringify(cls)}`);
    }
    for (const always of ALWAYS_CLASSES) {
        if (!cls.includes(always)) {
            failures.push(`branch ${name}: missing cell class ${JSON.stringify(always)}`);
        }
    }
}

// Assertion 1, reported separately so a regression is unambiguous.
if (innerHTMLAttempts > 0) {
    failures.push(`innerHTML was assigned ${innerHTMLAttempts} time(s)`);
}

if (failures.length) {
    console.error(`FAIL (${failures.length}) in ${MODULE}`);
    for (const f of failures) console.error('  - ' + f);
    process.exit(1);
}
console.log(`PASS  ${PAYLOADS.length} payloads as text, ${BRANCHES.length} branches render, innerHTML never assigned  [${MODULE}]`);
