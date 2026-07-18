// Browser ground-truth for the invoice layout bugs. Renders the SAME two cases
// as examples/repro_invoice_layout.rs at azul's content width and reports the
// vertical geometry (in pt, top-down) so we can diff against printpdf.
//
// Usage: node ground_truth.mjs <chrome-path>
import puppeteer from 'puppeteer-core';

const CHROME = process.argv[2] || '/usr/bin/chromium';
const CONTENT_PX = Math.round(595.28 * 96 / 72); // A4 210mm -> 794px
const PX2PT = 72 / 96;

const INVOICE = `<html>
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
    <div><h1>INVOICE #2026-071</h1><p class="muted">Issued 2026-07-17 Due 2026-08-16</p></div>
  </div>
  <p><strong>Billed to</strong><br/>Ferris Crab GmbH<br/>Hafenstrasse 12, 20359 Hamburg</p>
  <table>
    <tr><th>Description</th><th>Qty</th><th>Unit</th><th>Amount</th></tr>
    <tr><td>PDF generation consulting</td><td>12 h</td><td>Tall line one<br/>line two</td><td>1440</td></tr>
    <tr><td>WASM integration</td><td>8 h</td><td>120</td><td>960</td></tr>
  </table>
  <p class="total">Total: 2760</p>
</body></html>`;

const BLOCK_CELL = `<html><head><style>
  table { border-collapse: collapse; }
  td { border: 1px solid #999; padding: 0; vertical-align: middle; }
</style></head><body>
<table>
  <tr>
    <td><p>short</p></td>
    <td><p>line one<br/>line two<br/>line three</p></td>
  </tr>
</table>
</body></html>`;

const EXTRACT = () => {
  function textRect(substr) {
    const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let n;
    while ((n = walk.nextNode())) {
      const i = n.textContent.indexOf(substr);
      if (i >= 0) {
        const r = document.createRange();
        r.setStart(n, i); r.setEnd(n, i + substr.length);
        const b = r.getBoundingClientRect();
        return { top: b.top, bottom: b.bottom, left: b.left, mid: (b.top + b.bottom) / 2, h: b.height };
      }
    }
    return null;
  }
  return { textRect };
};

const browser = await puppeteer.launch({
  executablePath: CHROME, headless: true,
  args: ['--no-sandbox', '--disable-setuid-sandbox', '--force-device-scale-factor=1'],
});
const pt = (v) => (v == null ? null : +(v * PX2PT).toFixed(2));

async function render(html, want) {
  const page = await browser.newPage();
  await page.setViewport({ width: CONTENT_PX, height: 1400, deviceScaleFactor: 1 });
  // Force STANDARDS mode (CSS1Compat): azul always lays out in standards mode, but
  // a doctype-less document puts Chromium in quirks mode (BackCompat), which changes
  // body/first-child margin collapsing and silently skews the ground truth.
  const doc = html.trimStart().toLowerCase().startsWith('<!doctype')
    ? html
    : `<!DOCTYPE html>${html}`;
  await page.setContent(doc, { waitUntil: 'networkidle0' });
  const res = await page.evaluate((wants) => {
    const walk = (substr) => {
      const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      let n;
      while ((n = w.nextNode())) {
        const i = n.textContent.indexOf(substr);
        if (i >= 0) {
          const r = document.createRange();
          r.setStart(n, i); r.setEnd(n, i + substr.length);
          const b = r.getBoundingClientRect();
          return { top: b.top, mid: (b.top + b.bottom) / 2, h: b.height, left: b.left };
        }
      }
      return null;
    };
    const out = {};
    for (const s of wants) out[s] = walk(s);
    // also row boxes for the block-cell case
    const rows = [...document.querySelectorAll('tr')].map((tr) => {
      const b = tr.getBoundingClientRect();
      return { top: b.top, mid: (b.top + b.bottom) / 2, h: b.height };
    });
    return { out, rows };
  }, want);
  await page.close();
  return res;
}

console.log('=== INVOICE (browser, pt top-down) ===');
{
  const w = ['INVOICE', 'Issued', 'Billed to', 'Ferris Crab', 'Hafenstrasse',
             'PDF generation', '12 h', '1440', 'Tall line one', 'line two'];
  const { out } = await render(INVOICE, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(16)} top=${String(pt(r?.top)).padStart(8)}  mid=${String(pt(r?.mid)).padStart(8)}  h=${String(pt(r?.h)).padStart(6)}`);
  }
  console.log('  br steps: billed->ferris', pt(out['Ferris Crab'].top - out['Billed to'].top),
              ' ferris->hafen', pt(out['Hafenstrasse'].top - out['Ferris Crab'].top));
  const rowMid = (out['Tall line one'].top + out['line two'].mid + out['line two'].h / 2) / 2; // approx
  console.log('  tall-row single-cell mid vs tall-cell center:',
              '12h', pt(out['12 h'].mid - ((out['Tall line one'].top + (out['line two'].top + out['line two'].h)) / 2)));
}

console.log('\n=== BLOCK_CELL (browser, pt top-down) ===');
{
  const w = ['short', 'line one', 'line two', 'line three'];
  const { out, rows } = await render(BLOCK_CELL, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(12)} top=${String(pt(r?.top)).padStart(8)}  mid=${String(pt(r?.mid)).padStart(8)}`);
  }
  console.log('  row box:', 'top', pt(rows[0].top), 'mid', pt(rows[0].mid), 'h', pt(rows[0].h));
  const tallMid = (out['line one'].top + (out['line three'].top + out['line three'].h)) / 2;
  console.log('  "short" mid', pt(out['short'].mid), ' vs tall-cell content center', pt(tallMid),
              ' delta', pt(out['short'].mid - tallMid), '(~0 = short is centered)');
}

const PRIMITIVES = `<html><head><style>
  .box { padding: 20px; }
  .mt { margin-top: 30px; }
  .center { text-align: center; }
  .right { text-align: right; }
</style></head><body>
  <div class="box"><p>PadTopLine</p><p class="mt">MarginTop30</p></div>
  <p class="center">CenteredText</p>
  <p class="right">RightText</p>
</body></html>`;

console.log('\n=== PRIMITIVES (browser, pt top-down; left in pt) ===');
{
  const w = ['PadTopLine', 'MarginTop30', 'CenteredText', 'RightText'];
  const { out } = await render(PRIMITIVES, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(14)} top=${String(pt(r?.top)).padStart(8)}  mid=${String(pt(r?.mid)).padStart(8)}  left=${String(pt(r?.left)).padStart(8)}`);
  }
  console.log('  gap PadTop->MarginTop30:', pt(out['MarginTop30'].top - out['PadTopLine'].top), '(pad20 + margin30 collapse = 30 over content)');
}

const PRIM2 = `<html><head><style>
  body { font-family: Helvetica; }
  .bp { border: 5px solid; padding: 10px; }
  .col { display:flex; flex-direction:column; }
  .ib { display:inline-block; width:80px; }
</style></head><body>
  <div class="bp"><p>BorderPad</p></div>
  <div class="col"><div>ColA</div><div>ColB</div></div>
  <p><span class="ib">IBone</span><span class="ib">IBtwo</span>EndText</p>
</body></html>`;

console.log('\n=== PRIM2 (browser, pt top-down; left in pt) ===');
{
  const w = ['BorderPad', 'ColA', 'ColB', 'IBone', 'IBtwo', 'EndText'];
  const { out } = await render(PRIM2, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(10)} top=${String(pt(r?.top)).padStart(8)}  left=${String(pt(r?.left)).padStart(8)}`);
  }
  console.log('  IB packing: IBone->IBtwo', pt(out['IBtwo'].left - out['IBone'].left), '(=80px=60pt) IBtwo->End', pt(out['EndText'].left - out['IBtwo'].left));
  console.log('  col stack: ColA->ColB', pt(out['ColB'].top - out['ColA'].top));
}

const FLOATC = `<html><head><style>
  body { font-family: Helvetica; }
  .fl { float: left; width: 100px; height: 40px; }
</style></head><body>
  <div class="fl">FloatBox</div>
  <p>WrapLineOne beside the float here more words padding padding padding padding padding BelowFloatLine after the float clears down below the box area now continuing</p>
</body></html>`;

const ABSC = `<html><head><style>
  body { font-family: Helvetica; }
  .rel { position: relative; height: 100px; }
  .abs { position: absolute; top: 20px; left: 30px; }
</style></head><body>
  <div class="rel"><span class="abs">AbsPositioned</span>NormalFlow</div>
</body></html>`;

console.log('\n=== FLOATC (browser, pt top-down; left in pt) ===');
{
  const w = ['FloatBox', 'WrapLineOne', 'BelowFloatLine'];
  const { out } = await render(FLOATC, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(14)} top=${String(pt(r?.top)).padStart(8)}  left=${String(pt(r?.left)).padStart(8)}`);
  }
  console.log('  WrapLineOne left should be ~beside float (>=75pt); BelowFloatLine left should be ~6pt (cleared under float)');
}

console.log('\n=== ABSC (browser, pt top-down; left in pt) ===');
{
  const w = ['AbsPositioned', 'NormalFlow'];
  const { out } = await render(ABSC, w);
  for (const k of w) {
    const r = out[k];
    console.log(`  ${k.padEnd(14)} top=${String(pt(r?.top)).padStart(8)}  left=${String(pt(r?.left)).padStart(8)}`);
  }
  console.log('  AbsPositioned should be at rel.top+20px(15pt+6body=21pt), left 30px+6body? actually left=30px=22.5pt from rel');
}

await browser.close();
