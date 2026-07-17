// printpdf wasm playground.
//
// Everything talks to the wasm module through one JSON envelope:
//   { status: 0 | 1 | 2 | 3, data: <payload or error string> }
// (0 ok, 1 bad input JSON, 2 operation failed, 3 output unserializable)
//
// Wire-format notes (these bit us before - see the 0.12 demo rework):
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

// Decode a RawImage's pixel payload to 8-bit bytes (handles the base64 and the
// legacy array wire formats; 16-bit/float are downconverted).
function pixelBytesU8(px) {
    const fromB64 = s => { const b = atob(s); const a = new Uint8Array(b.length); for (let i = 0; i < b.length; i++) a[i] = b.charCodeAt(i); return a; };
    if (px.u8b64 != null) return fromB64(px.u8b64);
    if (px.u8 != null) return Uint8Array.from(px.u8);
    if (px.u16b64 != null) { const b = fromB64(px.u16b64); const a = new Uint8Array(b.length >> 1); for (let i = 0; i < a.length; i++) a[i] = b[i * 2 + 1]; return a; }
    if (px.u16 != null) return Uint8Array.from(px.u16, v => v >> 8);
    if (px.f32 != null) return Uint8Array.from(px.f32, v => Math.max(0, Math.min(255, v * 255)));
    return new Uint8Array();
}

// Encode a RawImage to a PNG blob in the browser (canvas), no wasm round-trip.
function rawImageToPngBlob(img) {
    const w = img.width, h = img.height;
    const src = pixelBytesU8(img.pixels);
    const fmt = String(img.data_format ?? img.dataFormat ?? 'rgba8').toLowerCase();
    let nc, alpha, bgr;
    if (fmt.startsWith('rgba')) { nc = 4; alpha = true; bgr = false; }
    else if (fmt.startsWith('bgra')) { nc = 4; alpha = true; bgr = true; }
    else if (fmt.startsWith('rgb')) { nc = 3; alpha = false; bgr = false; }
    else if (fmt.startsWith('bgr')) { nc = 3; alpha = false; bgr = true; }
    else if (fmt.startsWith('rg')) { nc = 2; alpha = true; bgr = false; }
    else { nc = 1; alpha = false; bgr = false; }
    const rgba = new Uint8ClampedArray(w * h * 4);
    for (let i = 0, p = 0; i < w * h; i++, p += nc) {
        let r, g, b, a = 255;
        if (nc >= 3) { if (bgr) { b = src[p]; g = src[p + 1]; r = src[p + 2]; } else { r = src[p]; g = src[p + 1]; b = src[p + 2]; } if (alpha) a = src[p + 3]; }
        else if (nc === 2) { r = g = b = src[p]; a = src[p + 1]; }
        else { r = g = b = src[p]; }
        const o = i * 4; rgba[o] = r; rgba[o + 1] = g; rgba[o + 2] = b; rgba[o + 3] = a;
    }
    const c = document.createElement('canvas'); c.width = w; c.height = h;
    c.getContext('2d').putImageData(new ImageData(rgba, w, h), 0, 0);
    return new Promise(res => c.toBlob(res, 'image/png'));
}

// Line-number gutter for an editor textarea.
function syncGutter(editorId, gutterId) {
    const ed = $(editorId), g = $(gutterId);
    const n = ed.value.split('\n').length;
    if (g._n !== n) {
        g._n = n;
        let s = '';
        for (let i = 1; i <= n; i++) s += i + '\n';
        g.textContent = s;
    }
    g.scrollTop = ed.scrollTop;
}

function downloadB64(b64, name, mime) {
    const bin = atob(b64);
    const arr = Uint8Array.from(bin, c => c.charCodeAt(0));
    download(new Blob([arr], { type: mime }), name);
}

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
    view: 'pdf',        // 'pdf' (printpdf output) or 'html' (browser reference)
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
const bootError = (msg) => { showError(new Error("WASM failed to load: " + msg)); };

let DEFAULT_FONT_NAMES = [];
try {
    await init();
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
    console.log('no default-fonts.js (dev mode) - HTML examples fall back to generic families');
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
  <p class="muted">Generated entirely in your browser by printpdf, no server involved.</p>
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
        <li>Eat immediately.</li>
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
  <h1>Q2 2026 Engineering Report</h1>
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
    } catch { /* dev without the asset - the example renders without the image */ }
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
    updateHtmlPreview();
    await refreshViewer();
}

// The browser's own rendering of the input HTML, as a reference for what the
// printpdf output should look like. Fonts are injected as @font-face so the
// reference uses the same faces the PDF embeds. Sandboxed: no scripts run.
function fontFaceCss() {
    // Only user-uploaded fonts: the standard defaults (Helvetica/Times/Courier)
    // have system equivalents, and inlining the ~9 MB default set (incl. 4.4 MB
    // NotoSansJP) into the iframe srcdoc stalled it to blank.
    return Object.entries(state.userFonts).map(([name, b64]) => {
        const fam = name.replace(/\.[a-z0-9]+$/i, '');
        return `@font-face{font-family:"${fam}";src:url(data:font/ttf;base64,${b64});}`;
    }).join('');
}
function updateHtmlPreview() {
    let html = $('html-editor').value;
    // Inline uploaded images: the sandboxed iframe can't resolve the relative
    // src names the PDF path resolves from state.userImages.
    for (const [name, b64] of Object.entries(state.userImages)) {
        const mime = /\.png$/i.test(name) ? 'image/png'
            : /\.jpe?g$/i.test(name) ? 'image/jpeg'
            : /\.gif$/i.test(name) ? 'image/gif' : 'application/octet-stream';
        const uri = `data:${mime};base64,${b64}`;
        html = html.split(`"${name}"`).join(`"${uri}"`).split(`'${name}'`).join(`'${uri}'`);
    }
    const doc = `<style>${fontFaceCss()}body{margin:16px;}</style>${html}`;
    $('html-preview').srcdoc = doc;
}
function setPreviewView(view) {
    state.view = view;
    $('pdf-viewer').hidden = view !== 'pdf';
    $('html-preview').hidden = view !== 'html';
    document.querySelectorAll('#view-toggle button').forEach(b =>
        b.classList.toggle('active', b.dataset.view === view));
    if (view === 'html') updateHtmlPreview();
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
        // minimap - that doubles renderer memory and can stall the tab.
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
        el.textContent = `${layer?.name ?? id}`;
        layers.appendChild(el);
    }
    if (!layers.children.length) layers.innerHTML = '<div class="hint">No layers in this document.</div>';

    for (const [, bm] of Object.entries(state.doc?.bookmarks?.map ?? state.doc?.bookmarks ?? {})) {
        const el = document.createElement('div');
        el.className = 'sidebar-item';
        // Bookmark shape is { name, page } - page is 1-based.
        el.textContent = `${bm?.name ?? 'bookmark'} (p. ${bm?.page ?? '?'})`;
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
// The editor shows the document STRUCTURE with the multi-MB font/image byte
// payloads replaced by short placeholders, so it stays small and hand-editable
// (add layers, bookmarks, ops, ...). The real bytes live in `state.doc` and are
// re-injected by id when the edited JSON is parsed back.
const FONT_STUB = '@@font (bytes elided; extract via the list above)';
const imgStub = x => `@@image ${x?.data?.width ?? '?'}x${x?.data?.height ?? '?'} (bytes elided)`;

function stripResources(doc) {
    const d = JSON.parse(JSON.stringify(doc));
    for (const [id] of mapEntries(d.resources?.fonts)) mapInsert(d.resources.fonts, id, FONT_STUB);
    for (const [id, x] of mapEntries(d.resources?.xobjects)) {
        if (x && x.type === 'image') mapInsert(d.resources.xobjects, id, { ...x, data: imgStub(x) });
    }
    return d;
}
function restoreResources(edited, full) {
    const fonts = new Map(mapEntries(full?.resources?.fonts));
    for (const [id, v] of mapEntries(edited.resources?.fonts)) {
        if (typeof v === 'string' && v.startsWith('@@font') && fonts.has(id)) mapInsert(edited.resources.fonts, id, fonts.get(id));
    }
    const xobj = new Map(mapEntries(full?.resources?.xobjects));
    for (const [id, x] of mapEntries(edited.resources?.xobjects)) {
        if (x && x.type === 'image' && typeof x.data === 'string' && x.data.startsWith('@@image') && xobj.has(id)) {
            mapInsert(edited.resources.xobjects, id, xobj.get(id));
        }
    }
    return edited;
}

const scheduleDocRender = debounce(() => render(refreshViewer), 400);
const isResourceStub = v => typeof v === 'string' && (v.startsWith('@@font') || v.startsWith('@@image'));
function getByPath(p) { let o = state.doc; for (const k of p) o = o?.[k]; return o; }
function setByPath(p, v) { let o = state.doc; for (let i = 0; i < p.length - 1; i++) o = o[p[i]]; o[p[p.length - 1]] = v; scheduleDocRender(); }
function delByPath(p) {
    let o = state.doc; for (let i = 0; i < p.length - 1; i++) o = o[p[i]];
    const k = p[p.length - 1];
    if (Array.isArray(o)) o.splice(k, 1); else delete o[k];
    renderJsonTree(); scheduleDocRender();
}
function coerceLike(str, orig) {
    if (typeof orig === 'number') { const n = Number(str); return Number.isNaN(n) ? orig : n; }
    if (typeof orig === 'boolean') return str === 'true';
    if (orig === null) return str === 'null' ? null : str;
    return str;
}
function addChild(path) {
    const c = getByPath(path);
    if (Array.isArray(c)) {
        c.push(c.length ? JSON.parse(JSON.stringify(c[c.length - 1])) : '');
    } else if (c && typeof c === 'object') {
        const k = prompt('New key');
        if (!k) return;
        c[k] = '';
    }
    renderJsonTree(); scheduleDocRender();
}
// Collapsible, editable tree over the document STRUCTURE (resource bytes shown
// as stubs). Leaf edits and add/remove mutate state.doc by path.
function treeNode(value, path, key) {
    if (value !== null && typeof value === 'object' && !isResourceStub(value)) {
        const isArr = Array.isArray(value);
        const det = document.createElement('details');
        det.className = 'tnode';
        if (path.length <= 1) det.open = true;
        const sum = document.createElement('summary');
        sum.innerHTML = `<span class="tkey">${escapeHtml(String(key))}</span>` +
            `<span class="tmeta">${isArr ? `[${value.length}]` : `{${Object.keys(value).length}}`}</span>`;
        const add = document.createElement('button');
        add.className = 'tbtn'; add.textContent = '+'; add.title = 'Add child';
        add.addEventListener('click', e => { e.preventDefault(); addChild(path); });
        sum.appendChild(add);
        det.appendChild(sum);
        const entries = isArr ? value.map((v, i) => [i, v]) : Object.entries(value);
        for (const [k, v] of entries) det.appendChild(treeNode(v, [...path, k], k));
        return det;
    }
    const row = document.createElement('div');
    row.className = 'tleaf';
    row.innerHTML = `<span class="tkey">${escapeHtml(String(key))}</span>`;
    if (isResourceStub(value)) {
        const s = document.createElement('span'); s.className = 'tstub'; s.textContent = value;
        row.appendChild(s);
    } else {
        const inp = document.createElement('input');
        inp.className = 'tval';
        inp.value = value === null ? 'null' : String(value);
        inp.addEventListener('change', () => setByPath(path, coerceLike(inp.value, value)));
        row.appendChild(inp);
    }
    if (path.length) {
        const del = document.createElement('button');
        del.className = 'tbtn tdel'; del.textContent = '×'; del.title = 'Remove';
        del.addEventListener('click', () => delByPath(path));
        row.appendChild(del);
    }
    return row;
}
function renderJsonTree() {
    const root = $('json-tree');
    root.innerHTML = '';
    if (state.doc) root.appendChild(treeNode(stripResources(state.doc), [], 'document'));
}

function syncJsonEditor() {
    renderJsonTree();
    const summary = $('parse-summary');
    if (state.doc) {
        summary.hidden = false;
        const fonts = mapEntries(state.doc.resources?.fonts).length;
        const xobjs = mapEntries(state.doc.resources?.xobjects).length;
        summary.textContent =
            `${state.doc.pages?.length ?? 0} pages, ${fonts} fonts, ${xobjs} xobjects`;
    } else {
        summary.hidden = true;
    }
    renderPdfResources();
}

// Font/image extraction: list embedded fonts and images from the parsed doc,
// each downloadable. Fonts come out as their original sfnt (data URI), images
// are re-encoded to PNG through the wasm encoder.
function renderPdfResources() {
    const box = $('pdf-resources');
    box.innerHTML = '';
    const doc = state.doc;
    const fonts = doc ? mapEntries(doc.resources?.fonts) : [];
    const images = (doc ? mapEntries(doc.resources?.xobjects) : [])
        .filter(([, x]) => x && x.type === 'image' && x.data);
    if (!fonts.length && !images.length) { box.hidden = true; return; }
    box.hidden = false;

    const row = (kind, id, onClick) => {
        const el = document.createElement('div');
        el.className = 'res-row';
        el.innerHTML = `<span class="res-kind">${kind}</span><span class="res-id">${escapeHtml(id)}</span>`;
        const btn = document.createElement('button');
        btn.className = 'ghost';
        btn.textContent = 'Save';
        btn.addEventListener('click', onClick);
        el.appendChild(btn);
        box.appendChild(el);
    };

    for (const [id, val] of fonts) {
        const b64 = typeof val === 'string' ? (val.split(',', 2)[1] ?? '') : '';
        row('font', id, () => downloadB64(b64, `${id}.ttf`, 'font/ttf'));
    }
    for (const [id, x] of images) {
        row('image', id, async () => {
            try { download(await rawImageToPngBlob(x.data), `${id}.png`); }
            catch (e) { showError(e); }
        });
    }
}

// ---------------------------------------------------------------- events: tabs & theme
document.querySelectorAll('.tabbar .tab').forEach(btn => {
    btn.addEventListener('click', () => {
        document.querySelectorAll('.tabbar .tab').forEach(b => b.classList.toggle('active', b === btn));
        for (const id of ['html-to-pdf-tab', 'parse-edit-pdf-tab', 'sign-pdf-tab']) {
            $(id).hidden = id !== `${btn.dataset.tab}-tab`;
        }
        // The HTML reference view only exists for the HTML input tab; elsewhere
        // there is no source HTML, so force the PDF preview.
        const isHtml = btn.dataset.tab === 'html-to-pdf';
        $('view-toggle').hidden = !isHtml;
        if (!isHtml) setPreviewView('pdf');
    });
});

// PDF / HTML preview toggle.
document.querySelectorAll('#view-toggle button').forEach(btn => {
    btn.addEventListener('click', () => setPreviewView(btn.dataset.view));
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
    syncGutter('html-editor', 'html-gutter');
    debouncedHtmlRender();
});

// Keep the HTML editor's line-number gutter in sync while typing and scrolling.
on('html-editor', 'input', () => syncGutter('html-editor', 'html-gutter'));
on('html-editor', 'scroll', () => { $('html-gutter').scrollTop = $('html-editor').scrollTop; });

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

// Save / load the document JSON itself (full, with embedded resources, so it
// round-trips back to a renderable document).
on('save-json', 'click', () => {
    if (!state.doc) { showError(new Error('Nothing to save yet.')); return; }
    download(new Blob([JSON.stringify(state.doc)], { type: 'application/json' }), 'document.json');
});
on('load-json', 'click', () => $('json-upload').click());
on('json-upload', 'change', async e => {
    const f = e.target.files[0];
    e.target.value = '';
    if (!f) return;
    try {
        state.doc = JSON.parse(await f.text());
        state.warnings = [];
        state.page = 1;
        syncJsonEditor();
        await render(refreshViewer);
    } catch (err) { showError(new Error(`document JSON: ${err.message ?? err}`)); }
});
on('sign-upload-pdf', 'click', () => $('pdf-file-upload').click());
on('pdf-file-upload', 'change', uploadPdf);

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
        // image_optimization re-encodes embedded images (Auto codec: JPEG for
        // photos, alpha preserved via SMask). Without it a single photo embeds
        // as multi-MB raw Flate pixels; the recipe example went 6 MB -> ~150 KB.
        options: { subset_fonts: true, image_optimization: { quality: 0.85, auto_optimize: true } },
    });
    const b64 = typeof data.bytes === 'string' ? data.bytes : null;
    const bin = b64 ? atob(b64) : String.fromCharCode(...data.bytes);
    const arr = Uint8Array.from(bin, c => c.charCodeAt(0));
    download(new Blob([arr], { type: 'application/pdf' }), 'document.pdf');
}));

// ---------------------------------------------------------------- go
$('html-editor').value = EXAMPLES.invoice;
syncGutter('html-editor', 'html-gutter');
$('view-toggle').hidden = false; // default tab is HTML input
render(renderHtmlTab);

// Test hook: the headless e2e suite (tests/e2e/) asserts on internal state.
window.__printpdf_state = state;
