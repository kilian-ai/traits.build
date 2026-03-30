#!/usr/bin/env node
// Quick check: parse the inlined scripts in index.html for JS syntax errors
const fs = require('fs');
const html = fs.readFileSync('index.html', 'utf8');

const scripts = [
    { name: 'wasm-runtime', tag: 'inline:wasm-runtime' },
    { name: 'sdk-runtime', tag: 'inline:sdk-runtime' },
    { name: 'terminal-runtime', tag: 'inline:terminal-runtime' },
    { name: 'bootloader', tag: null },
];

for (const s of scripts) {
    let code;
    if (s.tag) {
        const re = new RegExp(`data-runtime-src="${s.tag}">([\\s\\S]*?)<\\/script>`);
        const m = html.match(re);
        if (!m) { console.log(`[${s.name}] NOT FOUND`); continue; }
        code = m[1].trim();
    } else {
        // Last <script> block (bootloader)
        const all = html.match(/<script>([\s\S]*?)<\/script>/g);
        if (!all) { console.log(`[${s.name}] NOT FOUND`); continue; }
        code = all[all.length - 1].replace(/<\/?script>/g, '').trim();
    }
    try {
        new Function(code);
        console.log(`[${s.name}] OK (${code.length} chars)`);
    } catch(e) {
        console.log(`[${s.name}] PARSE ERROR: ${e.message}`);
        // Try to find error location
        const lines = code.split('\n');
        console.log(`  Total lines: ${lines.length}`);
    }
}
