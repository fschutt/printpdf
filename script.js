// printpdf wasm playground.
//
// Everything talks to the wasm module through one JSON envelope:
//   { status: 0 | 1 | 2 | 3, data: <payload or error string> }
// (0 ok, 1 bad input JSON, 2 operation failed, 3 output unserializable)
//
// Wire-format notes (these bit us before — see the 0.12 demo rework):
//   - GeneratePdfOptions fields are snake_case: page_width, margin_top, ...
//   - PdfSaveOptions fields are snake_case: subset_fonts, image_optimization
//   - XObjectTransform fields are snake_case: translate_x, scale_x, ... (Pt = plain number)
//   - ops are { type: "kebab-case", data: {...} }
//   - bookmarks are { name, page }
import init, {
    Pdf_HtmlToDocument,
    Pdf_BytesToDocument,
    Pdf_PageToSvg,
    Pdf_DocumentToBytes,
    Pdf_ResourcesForPage,
    Pdf_RegisterFonts,
    Pdf_DecodeImage,
} from './pkg/printpdf.js';

const $ = id => document.getElementById(id);
const on = (id, ev, fn) => $(id).addEventListener(ev, fn);

// ---------------------------------------------------------------- state
const state = {
    doc: null,          // current PdfDocument (JSON object)
    warnings: [],       // accumulated PdfWarnMsg
    page: 1,
    userFonts: {},      // name -> base64 (sent per render; defaults are registered once)
    userImages: {},     // name -> base64
    signatureCount: 0,
    rendering: false,
    renderQueued: false,
    // Monotonic counters/flags for the e2e suite (and anyone debugging):
    // renderCount increments after every completed viewer refresh;
    // signatureReady flips when the uploaded signature image finished decoding.
    renderCount: 0,
    signatureReady: false,
};

// ---------------------------------------------------------------- helpers
async function api(fn, payload) {
    const raw = await fn(JSON.stringify(payload));
    let envelope;
    try {
        envelope = JSON.parse(raw);
    } catch (e) {
        throw new Error(`API returned non-JSON (${e}): ${String(raw).slice(0, 200)}`);
    }
    if (envelope.status !== 0) {
        throw new Error(typeof envelope.data === 'string' ? envelope.data : JSON.stringify(envelope.data));
    }
    return envelope.data;
}

function debounce(fn, ms) {
    let t;
    return (...args) => { clearTimeout(t); t = setTimeout(() => fn(...args), ms); };
}

function b64FromFile(file) {
    return new Promise((resolve, reject) => {
        const r = new FileReader();
        r.onload = () => resolve(r.result.split(',', 2)[1]);
        r.onerror = reject;
        r.readAsDataURL(file);
    });
}

function showError(err) {
    const bar = $('viewer-error');
    if (!err) { bar.hidden = true; bar.textContent = ''; return; }
    bar.hidden = false;
    bar.textContent = String(err.message ?? err);
}

// Some resource collections serialize as { "id": ... } directly, others wrap in
// { map: { "id": ... } }. Copy entries for `ids` whichever shape we see.
function filterMap(container, ids) {
    if (!container) return container;
    const inner = container.map && typeof container.map === 'object' ? container.map : container;
    const picked = {};
    for (const id of ids) if (id in inner) picked[id] = inner[id];
    return container.map && typeof container.map === 'object' ? { map: picked } : picked;
}
function mapEntries(container) {
    if (!container) return [];
    const inner = container.map && typeof container.map === 'object' ? container.map : container;
    return Object.entries(inner);
}
function mapInsert(container, id, value) {
    const inner = container.map && typeof container.map === 'object' ? container.map : container;
    inner[id] = value;
}

// ---------------------------------------------------------------- boot
const bootError = (msg) => {
    const badge = $('wasm-status');
    badge.textContent = 'wasm failed';
    badge.className = 'badge badge-error';
    badge.title = String(msg);
};

let DEFAULT_FONT_NAMES = [];
try {
    await init();
    $('wasm-status').textContent = 'wasm ready';
    $('wasm-status').className = 'badge badge-ok';
} catch (e) {
    bootError(e);
    throw e;
}

// Default fonts live in a generated file (see .github/workflows/static.yml and
// scripts/build-demo.sh) so script.js stays byte-identical between dev and
// deploy. Locally without the generated file we just have no default fonts.
let defaultFonts = {};
try {
    const mod = await import('./default-fonts.js');
    defaultFonts = mod.DEFAULT_FONTS ?? {};
} catch {
    console.log('no default-fonts.js (dev mode) — HTML examples fall back to generic families');
}
DEFAULT_FONT_NAMES = Object.keys(defaultFonts);
if (DEFAULT_FONT_NAMES.length) {
    // NOT inside the import-catch: a registration failure is a real error and
    // must be surfaced, not mistaken for "file not found" (that conflation hid
    // a wasm panic during the 0.12 rework).
    try {
        const r = await api(Pdf_RegisterFonts, { fonts: defaultFonts, replace: true });
        console.log(`registered ${r.registered} default fonts`);
    } catch (e) {
        console.error('default font registration failed:', e);
        showError(e);
    }
}

// ---------------------------------------------------------------- examples
const EXAMPLES = {
    invoice: `<html>
<head><style>
  body { font-family: Helvetica, sans-serif; color: #222; }
  .head { display: flex; justify-content: space-between; }
  h1 { color: #b7410e; margin: 0 0 4px 0; }
  .muted { color: #666; font-size: 10pt; }
  table { width: 100%; border-collapse: collapse; margin-top: 24px; }
  th { text-align: left; border-bottom: 2px solid #b7410e; padding: 6px 4px; }
  td { border-bottom: 1px solid #ddd; padding: 6px 4px; }
  .total { text-align: right; font-size: 14pt; margin-top: 16px; font-weight: bold; }
</style></head>
<body>
  <div class="head">
    <div><h1>INVOICE #2026-071</h1><p class="muted">Issued 2026-07-17 · Due 2026-08-16</p></div>
  </div>
  <p><strong>Billed to</strong><br/>Ferris Crab GmbH<br/>Hafenstraße 12, 20359 Hamburg</p>
  <table>
    <tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr>
    <tr><td>PDF generation consulting</td><td>12 h</td><td>€120</td><td>€1,440</td></tr>
    <tr><td>WASM integration</td><td>8 h</td><td>€120</td><td>€960</td></tr>
    <tr><td>Font subsetting support</td><td>3 h</td><td>€120</td><td>€360</td></tr>
  </table>
  <p class="total">Total: €2,760</p>
  <p class="muted">Generated entirely in your browser by printpdf — no server involved.</p>
</body></html>`,

    recipe: `<html>
<head><style>
  body { font-family: Helvetica, sans-serif; color: #2a2a2a; }
  h1 { color: #7a3e0e; border-bottom: 3px solid #7a3e0e; padding-bottom: 6px; }
  .cols { display: flex; gap: 24px; }
  .ing { background: #faf3e8; padding: 12px 16px; border-radius: 8px; min-width: 40%; }
  li { margin-bottom: 6px; }
  img { border-radius: 8px; }
</style></head>
<body>
  <h1>Weeknight Miso Ramen</h1>
  <p><em>Serves 2 · 25 minutes</em></p>
  <img src="cat.jpg" width="220" />
  <div class="cols">
    <div class="ing">
      <h3>Ingredients</h3>
      <ul>
        <li>2 portions fresh ramen noodles</li>
        <li>3 tbsp white miso</li>
        <li>1 l chicken or veggie stock</li>
        <li>2 soft-boiled eggs</li>
        <li>Spring onions, nori, sesame</li>
      </ul>
    </div>
    <div>
      <h3>Steps</h3>
      <ol>
        <li>Simmer the stock; whisk in the miso off the heat.</li>
        <li>Cook noodles 90 seconds; drain well.</li>
        <li>Assemble bowls: noodles, broth, toppings.</li>
        <li>Eat immediately — ramen waits for no one.</li>
      </ol>
    </div>
  </div>
</body></html>`,

    report: `<html>
<head><style>
  body { font-family: Helvetica, sans-serif; color: #1c2128; }
  h1 { font-size: 22pt; margin-bottom: 0; }
  .sub { color: #59636e; margin-top: 2px; }
  .kpis { display: flex; gap: 12px; margin: 18px 0; }
  .kpi { flex: 1; border: 1px solid #d7dce2; border-radius: 8px; padding: 10px 14px; }
  .kpi b { font-size: 16pt; display: block; }
  .up { color: #2e7d32; } .down { color: #c62828; }
  h2 { border-bottom: 1px solid #d7dce2; padding-bottom: 4px; margin-top: 22px; }
</style></head>
<body>
  <h1>Q2 2026 — Engineering Report</h1>
  <p class="sub">printpdf project · rendered from HTML in the browser</p>
  <div class="kpis">
    <div class="kpi"><span>Open issues</span><b class="down">6</b></div>
    <div class="kpi"><span>Issues closed</span><b class="up">14</b></div>
    <div class="kpi"><span>Test count</span><b class="up">220+</b></div>
  </div>
  <h2>Highlights</h2>
  <ul>
    <li>0.11.2 fixed installability for every consumer (#279).</li>
    <li>SVG pipeline rewritten: gradients, patterns and Acrobat compatibility.</li>
    <li>Custom fonts finally resolve in HTML rendering.</li>
    <li>Post-release CI verifies the published crate on a schedule.</li>
  </ul>
  <h2>Outlook</h2>
  <p>0.12 focuses on round-trip fidelity, multilevel outlines and named spot colors.
     The wasm demo you are looking at is itself part of the test suite now.</p>
</body></html>`,

    blank: `<html>\n<body>\n  <h1>Hello printpdf</h1>\n  <p>Edit me.</p>\n</body>\n</html>`,
};

// Try to provide the recipe image from the repo (works on Pages + dev server).
(async () => {
    try {
        const resp = await fetch('./examples/assets/img/cat.jpg');
        if (resp.ok) {
            const buf = new Uint8Array(await resp.arrayBuffer());
            let bin = '';
            for (const b of buf) bin += String.fromCharCode(b);
            state.userImages['cat.jpg'] = btoa(bin);
            renderResourceChips();
        }
    } catch { /* dev without the asset — the example renders without the image */ }
})();

// ---------------------------------------------------------------- render pipeline
const PAGE_SIZES = { a4: [210, 297], letter: [215.9, 279.4], a5: [148, 210] };

function currentOptions() {
    const [w, h] = PAGE_SIZES[$('page-size').value] ?? PAGE_SIZES.a4;
    const opts = { page_width: w, page_height: h };
    if ($('opt-page-numbers').checked) opts.show_page_numbers = true;
    return opts;
}

async function renderHtmlTab() {
    const html = $('html-editor').value;
    const data = await api(Pdf_HtmlToDocument, {
        html,
        images: state.userImages,
        fonts: state.userFonts,
        options: currentOptions(),
    });
    state.doc = data.doc;
    state.warnings = data.warnings ?? [];
    syncJsonEditor();
    await refreshViewer();
}

// Renders every page of state.doc to SVG and fills viewer + minimap.
async function refreshViewer() {
    const viewer = $('pdf-viewer');
    const minimap = $('minimap-view');
    if (!state.doc || !state.doc.pages?.length) {
        viewer.innerHTML = '<div class="placeholder">No document yet.<br/>Type some HTML or upload a PDF.</div>';
        minimap.innerHTML = '';
        $('page-count').textContent = '0';
        renderWarnings();
        return;
    }

    const pages = state.doc.pages;
    $('page-count').textContent = String(pages.length);
    state.page = Math.min(Math.max(1, state.page), pages.length);
    $('page-number').value = String(state.page);

    const svgs = [];
    for (const page of pages) {
        const res = await api(Pdf_ResourcesForPage, { page });
        const resources = {
            fonts: filterMap(state.doc.resources?.fonts ?? {}, res.fonts ?? []),
            xobjects: filterMap(state.doc.resources?.xobjects ?? {}, res.xobjects ?? []),
            // Small maps that resources_for_page doesn't enumerate: send whole.
            extgstates: state.doc.resources?.extgstates ?? {},
            shadings: state.doc.resources?.shadings ?? {},
            layers: state.doc.resources?.layers ?? {},
        };
        const out = await api(Pdf_PageToSvg, {
            page,
            resources,
            options: { imageFormats: ['png', 'jpeg'] },
        });
        state.warnings.push(...(out.warnings ?? []));
        svgs.push(out.svg);
    }

    viewer.innerHTML = '';
    minimap.innerHTML = '';
    svgs.forEach((svg, i) => {
        const pageEl = document.createElement('div');
        pageEl.className = 'page';
        pageEl.dataset.page = String(i + 1);
        pageEl.innerHTML = svg;
        viewer.appendChild(pageEl);

        const thumb = document.createElement('div');
        thumb.className = 'minimap-page' + (i + 1 === state.page ? ' current' : '');
        // Don't duplicate multi-MB SVGs (embedded fonts/images) into the
        // minimap — that doubles renderer memory and can stall the tab.
        thumb.innerHTML = (svg.length < 1_500_000 ? svg : '') +
            `<div class="pageno">${i + 1}</div>`;
        thumb.addEventListener('click', () => gotoPage(i + 1));
        minimap.appendChild(thumb);
    });

    renderSidebarMeta();
    renderWarnings();
    gotoPage(state.page, { scroll: false });
    state.renderCount++;
}

function renderSidebarMeta() {
    const layers = $('layers-view');
    const bookmarks = $('bookmarks-view');
    layers.innerHTML = '';
    bookmarks.innerHTML = '';

    for (const [id, layer] of mapEntries(state.doc?.resources?.layers)) {
        const el = document.createElement('div');
        el.className = 'sidebar-item';
        el.textContent = `◧ ${layer?.name ?? id}`;
        layers.appendChild(el);
    }
    if (!layers.children.length) layers.innerHTML = '<div class="hint">No layers in this document.</div>';

    for (const [, bm] of Object.entries(state.doc?.bookmarks?.map ?? state.doc?.bookmarks ?? {})) {
        const el = document.createElement('div');
        el.className = 'sidebar-item';
        // Bookmark shape is { name, page } — page is 1-based.
        el.textContent = `🔖 ${bm?.name ?? 'bookmark'} (p. ${bm?.page ?? '?'})`;
        el.addEventListener('click', () => gotoPage(Number(bm?.page ?? 1)));
        bookmarks.appendChild(el);
    }
    if (!bookmarks.children.length) bookmarks.innerHTML = '<div class="hint">No bookmarks.</div>';
}

function renderWarnings() {
    const badge = $('warnings-badge');
    const drawer = $('warnings-drawer');
    const list = state.warnings ?? [];
    badge.hidden = list.length === 0;
    badge.textContent = `${list.length} warning${list.length === 1 ? '' : 's'}`;
    drawer.innerHTML = list.map(w => {
        const sev = String(w.severity ?? 'info').toLowerCase();
        return `<div class="w"><span class="sev ${sev.includes('err') ? 'error' : ''}">${sev}</span>` +
               `<span>p.${w.page ?? 0}</span><span>${escapeHtml(w.msg ?? '')}</span></div>`;
    }).join('');
}

const escapeHtml = s => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

function gotoPage(n, { scroll = true } = {}) {
    if (!state.doc?.pages?.length) return;
    state.page = Math.min(Math.max(1, n), state.doc.pages.length);
    $('page-number').value = String(state.page);
    document.querySelectorAll('.minimap-page').forEach((el, i) =>
        el.classList.toggle('current', i + 1 === state.page));
    if (scroll) {
        document.querySelector(`.pdf-viewer .page[data-page="${state.page}"]`)
            ?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
}

// Serialize renders: never run two at once, coalesce bursts.
async function render(tabRenderer) {
    if (state.rendering) { state.renderQueued = tabRenderer; return; }
    state.rendering = true;
    showError(null);
    try {
        await tabRenderer();
    } catch (e) {
        showError(e);
    } finally {
        state.rendering = false;
        if (state.renderQueued) {
            const next = state.renderQueued;
            state.renderQueued = null;
            render(next);
        }
    }
}

// ---------------------------------------------------------------- JSON editor sync
function syncJsonEditor() {
    const editor = $('json-editor');
    if (document.activeElement === editor) return; // don't clobber typing
    // Multi-megabyte documents (full embedded fonts/images) freeze the tab if
    // pretty-printed into a textarea wholesale — compact-print large ones and
    // hard-cap the editor payload.
    let text = '';
    if (state.doc) {
        const compact = JSON.stringify(state.doc);
        text = compact.length > 4_000_000
            ? `// document JSON is ${(compact.length / 1e6).toFixed(1)} MB — too large to edit here.\n` +
              `// Use Download PDF for the full document.\n`
            : compact.length > 500_000
                ? compact
                : JSON.stringify(state.doc, null, 2);
    }
    editor.value = text;
    const summary = $('parse-summary');
    if (state.doc) {
        summary.hidden = false;
        const fonts = mapEntries(state.doc.resources?.fonts).length;
        const xobjs = mapEntries(state.doc.resources?.xobjects).length;
        summary.textContent =
            `${state.doc.pages?.length ?? 0} page(s) · ${fonts} font(s) · ${xobjs} xobject(s) · ` +
            `title: ${state.doc.metadata?.info?.document_title || state.doc.metadata?.info?.title || '(none)'}`;
    } else {
        summary.hidden = true;
    }
}

// ---------------------------------------------------------------- events: tabs & theme
document.querySelectorAll('.tabbar .tab').forEach(btn => {
    btn.addEventListener('click', () => {
        document.querySelectorAll('.tabbar .tab').forEach(b => b.classList.toggle('active', b === btn));
        for (const id of ['html-to-pdf-tab', 'parse-edit-pdf-tab', 'sign-pdf-tab']) {
            $(id).hidden = id !== `${btn.dataset.tab}-tab`;
        }
    });
});

// Sidebar mode switcher (Pages / Layers / Bookmarks). These buttons had no
// handler, so the Layers and Bookmarks panels were unreachable.
document.querySelectorAll('.sidebar-modes button').forEach(btn => {
    btn.addEventListener('click', () => {
        const mode = btn.dataset.mode;
        document.querySelectorAll('.sidebar-modes button').forEach(b => b.classList.toggle('active', b === btn));
        for (const m of ['minimap', 'layers', 'bookmarks']) {
            $(`${m}-view`).hidden = m !== mode;
        }
    });
});

on('theme-toggle', 'click', () => {
    const root = document.documentElement;
    const dark = matchMedia('(prefers-color-scheme: dark)').matches;
    const cur = root.dataset.theme || (dark ? 'dark' : 'light');
    root.dataset.theme = cur === 'dark' ? 'light' : 'dark';
    try { localStorage.setItem('printpdf-theme', root.dataset.theme); } catch { }
});
try {
    const saved = localStorage.getItem('printpdf-theme');
    if (saved) document.documentElement.dataset.theme = saved;
} catch { }

// ---------------------------------------------------------------- events: html tab
const debouncedHtmlRender = debounce(() => render(renderHtmlTab), 400);
on('html-editor', 'input', debouncedHtmlRender);
on('page-size', 'change', debouncedHtmlRender);
on('opt-page-numbers', 'change', debouncedHtmlRender);
on('html-examples', 'change', () => {
    $('html-editor').value = EXAMPLES[$('html-examples').value] ?? EXAMPLES.blank;
    debouncedHtmlRender();
});

function renderResourceChips() {
    const box = $('html-resources');
    box.innerHTML = '';
    const add = (name, kind, map) => {
        const chip = document.createElement('span');
        chip.className = 'chip';
        chip.innerHTML = `<span class="kind">${kind}</span> ${escapeHtml(name)} <button title="remove">×</button>`;
        chip.querySelector('button').addEventListener('click', () => {
            delete map[name];
            renderResourceChips();
            debouncedHtmlRender();
        });
        box.appendChild(chip);
    };
    Object.keys(state.userFonts).forEach(n => add(n, 'font', state.userFonts));
    Object.keys(state.userImages).forEach(n => add(n, 'img', state.userImages));
    box.hidden = box.children.length === 0;
}

on('add-font-html', 'click', () => $('font-upload').click());
on('font-upload', 'change', async e => {
    for (const f of e.target.files) state.userFonts[f.name] = await b64FromFile(f);
    e.target.value = '';
    renderResourceChips();
    debouncedHtmlRender();
});
on('add-image-html', 'click', () => $('image-upload').click());
on('image-upload', 'change', async e => {
    for (const f of e.target.files) state.userImages[f.name] = await b64FromFile(f);
    e.target.value = '';
    renderResourceChips();
    debouncedHtmlRender();
});

on('save-config', 'click', () => {
    const blob = new Blob([JSON.stringify({
        html: $('html-editor').value,
        images: state.userImages,
        fonts: state.userFonts,
    }, null, 2)], { type: 'application/json' });
    download(blob, 'printpdf-config.json');
});
on('load-config', 'click', () => $('config-upload').click());
on('config-upload', 'change', async e => {
    const f = e.target.files[0];
    e.target.value = '';
    if (!f) return;
    try {
        const cfg = JSON.parse(await f.text());
        $('html-editor').value = cfg.html ?? '';
        state.userImages = cfg.images ?? {};
        state.userFonts = cfg.fonts ?? {};
        renderResourceChips();
        debouncedHtmlRender();
    } catch (err) { showError(err); }
});

// ---------------------------------------------------------------- events: parse tab
async function uploadPdf(fileInputEvent) {
    const f = fileInputEvent.target.files[0];
    fileInputEvent.target.value = '';
    if (!f) return;
    const bytes = await b64FromFile(f);
    await render(async () => {
        const data = await api(Pdf_BytesToDocument, { bytes });
        state.doc = data.doc;
        state.warnings = data.warnings ?? [];
        state.page = 1;
        syncJsonEditor();
        await refreshViewer();
    });
}
on('upload-pdf', 'click', () => $('pdf-file-upload').click());
on('sign-upload-pdf', 'click', () => $('pdf-file-upload').click());
on('pdf-file-upload', 'change', uploadPdf);

on('json-editor', 'input', debounce(() => {
    try {
        state.doc = JSON.parse($('json-editor').value);
        render(refreshViewer);
    } catch (e) {
        showError(new Error(`document JSON: ${e.message}`));
    }
}, 800));

// ---------------------------------------------------------------- events: sign tab
let signatureImage = null; // decoded RawImage JSON

on('signature-image-upload', 'change', async e => {
    const f = e.target.files[0];
    if (!f) return;
    try {
        const bytes = await b64FromFile(f);
        const data = await api(Pdf_DecodeImage, { bytes });
        signatureImage = data.image;
        state.signatureReady = true;
        console.log(`signature decoded: ${data.image.width}x${data.image.height}`);
        showError(null);
    } catch (err) {
        signatureImage = null;
        state.signatureReady = false;
        showError(err);
    }
});

on('apply-signature', 'click', () => render(async () => {
    console.log('apply-signature clicked');
    if (!state.doc) throw new Error('Upload a PDF first (button 1).');
    if (!signatureImage) throw new Error('Choose a signature image first (button 2).');
    const pageIdx = Number($('signature-page').value) - 1;
    const page = state.doc.pages?.[pageIdx];
    if (!page) throw new Error(`Page ${pageIdx + 1} does not exist.`);

    const id = `signature-${++state.signatureCount}`;
    state.doc.resources ??= {};
    state.doc.resources.xobjects ??= {};
    mapInsert(state.doc.resources.xobjects, id, { type: 'image', data: signatureImage });

    const scale = Number($('signature-scale').value) || 1;
    page.ops ??= [];
    page.ops.push({
        type: 'use-xobject',
        data: {
            id,
            transform: {
                translate_x: Number($('signature-x').value) || 0,
                translate_y: Number($('signature-y').value) || 0,
                scale_x: scale,
                scale_y: scale,
                dpi: 96,
            },
        },
    });

    state.page = pageIdx + 1;
    syncJsonEditor();
    await refreshViewer();
}));

// ---------------------------------------------------------------- events: viewer
on('prev-page', 'click', () => gotoPage(state.page - 1));
on('next-page', 'click', () => gotoPage(state.page + 1));
on('page-number', 'change', () => gotoPage(Number($('page-number').value)));
on('warnings-badge', 'click', () => { $('warnings-drawer').hidden = !$('warnings-drawer').hidden; });

function download(blob, name) {
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}

on('save-pdf', 'click', () => render(async () => {
    if (!state.doc) throw new Error('Nothing to save yet.');
    const data = await api(Pdf_DocumentToBytes, {
        doc: state.doc,
        options: { subset_fonts: true },
    });
    const b64 = typeof data.bytes === 'string' ? data.bytes : null;
    const bin = b64 ? atob(b64) : String.fromCharCode(...data.bytes);
    const arr = Uint8Array.from(bin, c => c.charCodeAt(0));
    download(new Blob([arr], { type: 'application/pdf' }), 'document.pdf');
}));

// ---------------------------------------------------------------- go
$('html-editor').value = EXAMPLES.invoice;
render(renderHtmlTab);

// Test hook: the headless e2e suite (tests/e2e/) asserts on internal state.
window.__printpdf_state = state;
