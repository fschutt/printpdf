// Headless e2e scenarios for the printpdf wasm demo. Each scenario drives the
// REAL page (no API shortcuts): typing, tab switches, file uploads, downloads.
// Run via driver.mjs; every scenario also fails on any console error it causes.
import { join } from 'node:path';
import { stat } from 'node:fs/promises';

const SEL = {
    page: '.pdf-viewer .page svg',
    errorBar: '#viewer-error',
};

async function waitRendered(page, minPages = 1, timeout = 90000) {
    await page.waitForFunction(
        (sel, n) => document.querySelectorAll(sel).length >= n,
        { timeout },
        SEL.page, minPages,
    );
    const err = await page.$eval(SEL.errorBar, el => el.hidden ? '' : el.textContent);
    if (err) throw new Error(`error bar visible: ${err.slice(0, 300)}`);
}

const stateOf = page => page.evaluate(() => ({
    pages: window.__printpdf_state.doc?.pages?.length ?? 0,
    fonts: (() => {
        const f = window.__printpdf_state.doc?.resources?.fonts ?? {};
        return Object.keys(f.map ?? f).length;
    })(),
    xobjects: (() => {
        const x = window.__printpdf_state.doc?.resources?.xobjects ?? {};
        return Object.keys(x.map ?? x).length;
    })(),
    warnings: window.__printpdf_state.warnings?.length ?? 0,
}));

// Captures the next browser download triggered inside `fn`, returns its path.
async function captureDownload(page, out, fn) {
    const client = await page.createCDPSession();
    await client.send('Browser.setDownloadBehavior', {
        behavior: 'allowAndName',
        downloadPath: out,
        eventsEnabled: true,
    });
    const guid = new Promise((resolve, reject) => {
        const t = setTimeout(() => reject(new Error('download did not finish in 60s')), 60000);
        client.on('Browser.downloadProgress', ev => {
            if (ev.state === 'completed') { clearTimeout(t); resolve(ev.guid); }
            if (ev.state === 'canceled') { clearTimeout(t); reject(new Error('download canceled')); }
        });
    });
    await fn();
    const path = join(out, await guid);
    if ((await stat(path)).size < 1000) throw new Error('downloaded PDF suspiciously small');
    return path;
}

let downloadedPdf = null;

export const scenarios = [
    {
        name: '01-boot',
        async run(page) {
            await page.waitForFunction(
                () => document.getElementById('wasm-status')?.classList.contains('badge-ok')
                    || document.getElementById('wasm-status')?.classList.contains('badge-error'),
                { timeout: 120000 },
            );
            const cls = await page.$eval('#wasm-status', el => el.className + ' | ' + el.title);
            if (cls.includes('badge-error')) throw new Error(`wasm failed to init: ${cls}`);
        },
    },
    {
        name: '02-invoice-renders-with-fonts',
        async run(page) {
            await waitRendered(page); // invoice example renders on boot
            const s = await stateOf(page);
            if (s.pages < 1) throw new Error(`expected >=1 page, got ${s.pages}`);
            if (s.fonts < 1) throw new Error(
                `expected embedded fonts (defaults are registered), got ${s.fonts}`);
        },
    },
    {
        name: '03-recipe-example-embeds-image',
        async run(page) {
            // Wait for a NEW render, not the still-satisfied invoice condition —
            // asserting on the old doc was a race that failed this scenario.
            const before = await page.evaluate(() => window.__printpdf_state.renderCount);
            await page.select('#html-examples', 'recipe');
            await page.waitForFunction(
                (n) => window.__printpdf_state.renderCount > n,
                { timeout: 90000 },
                before,
            );
            const s = await stateOf(page);
            if (s.pages < 1) throw new Error('recipe rendered no pages');
            if (s.xobjects < 1) throw new Error(
                `recipe references cat.jpg — expected >=1 xobject, got ${s.xobjects}`);
        },
    },
    {
        name: '04-download-pdf',
        async run(page, ctx) {
            downloadedPdf = await captureDownload(page, ctx.out, async () => {
                await page.click('#save-pdf');
            });
            console.log(`   downloaded: ${downloadedPdf}`);
        },
    },
    {
        name: '05-parse-edit-roundtrip',
        async run(page) {
            if (!downloadedPdf) throw new Error('no PDF from scenario 04');
            await page.click('#tab-parse');
            const input = await page.$('#pdf-file-upload');
            await input.uploadFile(downloadedPdf);
            await page.waitForFunction(
                // The summary bar is the truthful "parsed" signal — the JSON
                // editor may hold only a short size-cap notice for huge docs.
                () => !document.getElementById('parse-summary').hidden
                    && document.querySelectorAll('.pdf-viewer .page svg').length >= 1,
                { timeout: 90000 },
            );
            const s = await stateOf(page);
            if (s.pages < 1) throw new Error('re-parsed document has no pages');
            if (s.fonts < 1) throw new Error(
                'our own saved PDF must re-parse with its (subset) fonts — got 0 ' +
                '(regression: ParsedFont serde / subset re-parse)');
        },
    },
    {
        name: '06-sign-pdf',
        async run(page, ctx) {
            await page.click('#tab-sign');
            const sig = await page.$('#signature-image-upload');
            await sig.uploadFile(join(ctx.root, 'tests/e2e/assets/signature.png'));
            // The decode is async (Pdf_DecodeImage) — clicking apply before it
            // finishes places nothing. Wait for the ready flag.
            await page.waitForFunction(
                () => window.__printpdf_state.signatureReady === true,
                { timeout: 60000 },
            );
            await page.click('#apply-signature');
            await page.waitForFunction(
                () => {
                    const doc = window.__printpdf_state.doc;
                    if (!doc) return false;
                    const ops = doc.pages?.[0]?.ops ?? [];
                    return ops.some(o => o.type === 'use-xobject'
                        && String(o.data?.id ?? '').startsWith('signature-'));
                },
                { timeout: 90000 },
            );
        },
    },
    {
        name: '07-save-signed-pdf',
        async run(page, ctx) {
            const path = await captureDownload(page, ctx.out, async () => {
                await page.click('#save-pdf');
            });
            console.log(`   signed pdf: ${path}`);
        },
    },
];
