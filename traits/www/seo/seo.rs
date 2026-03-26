use maud::{html, DOCTYPE, PreEscaped};
use serde_json::Value;

pub fn seo(_args: &[Value]) -> Value {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { "traits.build — SEO Settings" }
                style { (PreEscaped(CSS)) }
            }
            body {
                div.page {
                    section.hero.card {
                        p.eyebrow { "seo" }
                        h1 { "Meta & Open Graph" }
                        p.subtitle {
                            "Configure search engine metadata, Open Graph tags, Twitter cards, and JSON-LD structured data. "
                            "Changes are persisted via " code { "sys.config" } " and applied on page load when a helper or server is connected."
                        }
                        div.badges {
                            span.badge id="connBadge" { "detecting..." }
                            span.badge { "sys.config" }
                        }
                    }

                    // Basic meta
                    section.card {
                        h2 { "Basic Meta" }
                        div.field {
                            label for="f_title" { "Page Title" }
                            input id="f_title" type="text" data-key="title" placeholder="traits.build — composable function kernel in Rust + WASM" {}
                        }
                        div.field {
                            label for="f_description" { "Description" }
                            textarea id="f_description" data-key="description" rows="3"
                                placeholder="Typed, composable functions compiled into a single Rust binary..." {}
                        }
                        div.field {
                            label for="f_keywords" { "Keywords" }
                            input id="f_keywords" type="text" data-key="keywords"
                                placeholder="rust, wasm, webassembly, function kernel, ..." {}
                        }
                        div.field {
                            label for="f_author" { "Author" }
                            input id="f_author" type="text" data-key="author" placeholder="Kilian" {}
                        }
                        div.field {
                            label for="f_canonical" { "Canonical URL" }
                            input id="f_canonical" type="url" data-key="canonical" placeholder="https://www.traits.build/" {}
                        }
                    }

                    // Open Graph
                    section.card {
                        h2 { "Open Graph" }
                        div.field {
                            label for="f_og_title" { "og:title" }
                            input id="f_og_title" type="text" data-key="og_title"
                                placeholder="traits.build — composable function kernel in Rust + WASM" {}
                        }
                        div.field {
                            label for="f_og_description" { "og:description" }
                            textarea id="f_og_description" data-key="og_description" rows="2"
                                placeholder="Typed, composable functions compiled into a single Rust binary..." {}
                        }
                        div.field {
                            label for="f_og_site_name" { "og:site_name" }
                            input id="f_og_site_name" type="text" data-key="og_site_name" placeholder="traits.build" {}
                        }
                        div.field {
                            label for="f_og_image" { "og:image" }
                            input id="f_og_image" type="url" data-key="og_image" placeholder="https://www.traits.build/og-image.png" {}
                        }
                    }

                    // Twitter
                    section.card {
                        h2 { "Twitter Card" }
                        div.field {
                            label for="f_twitter_title" { "twitter:title" }
                            input id="f_twitter_title" type="text" data-key="twitter_title"
                                placeholder="traits.build — composable function kernel" {}
                        }
                        div.field {
                            label for="f_twitter_description" { "twitter:description" }
                            textarea id="f_twitter_description" data-key="twitter_description" rows="2"
                                placeholder="Single Rust binary + WASM browser kernel..." {}
                        }
                        div.field {
                            label for="f_twitter_image" { "twitter:image" }
                            input id="f_twitter_image" type="url" data-key="twitter_image" placeholder="" {}
                        }
                    }

                    // JSON-LD
                    section.card {
                        h2 { "JSON-LD" }
                        p.note { "Structured data for Google rich results." }
                        div.field {
                            label for="f_jsonld_name" { "Application Name" }
                            input id="f_jsonld_name" type="text" data-key="jsonld_name" placeholder="traits.build" {}
                        }
                        div.field {
                            label for="f_jsonld_description" { "Application Description" }
                            textarea id="f_jsonld_description" data-key="jsonld_description" rows="2"
                                placeholder="Typed, composable functions compiled into a single Rust binary..." {}
                        }
                        div.field {
                            label for="f_jsonld_author" { "Author Name" }
                            input id="f_jsonld_author" type="text" data-key="jsonld_author" placeholder="Kilian" {}
                        }
                    }

                    // Actions
                    section.card {
                        div.actions-row {
                            button.primary id="btnSave" onclick="saveAll()" { "Save All" }
                            button id="btnApply" onclick="applyNow()" { "Apply to Page" }
                            button id="btnReset" onclick="resetAll()" { "Reset to Defaults" }
                        }
                        p.inline-status id="status" {}
                    }

                    // Preview
                    section.card {
                        h2 { "Preview" }
                        p.note { "Generated " code { "<head>" } " tags from current form values." }
                        pre.preview id="preview" {}
                    }

                    // Activity
                    section.card.log-card {
                        h2 { "Activity" }
                        div.log id="activityLog" {
                            span.entry { span.time { "[--:--:--]" } " Ready" }
                        }
                    }
                }

                script { (PreEscaped(JS)) }
            }
        }
    };

    Value::String(markup.into_string())
}

const CSS: &str = r##"
:root {
  --bg: #0b1115;
  --panel: #121b21;
  --panel-2: #17232c;
  --line: #243744;
  --text: #e7eef3;
  --muted: #9ab0bf;
  --accent: #59d5b0;
  --warn: #f5b942;
  --danger: #ef6b73;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background:
    radial-gradient(circle at top left, rgba(89, 213, 176, 0.08), transparent 28%),
    linear-gradient(180deg, #081015 0%, var(--bg) 100%);
  color: var(--text);
  font-family: "Iosevka Aile", "IBM Plex Sans", "Segoe UI", sans-serif;
}
.page {
  max-width: 800px;
  margin: 0 auto;
  padding: 32px 20px 48px;
}
.card {
  background: linear-gradient(180deg, rgba(23, 35, 44, 0.96), rgba(18, 27, 33, 0.96));
  border: 1px solid var(--line);
  border-radius: 18px;
  padding: 20px;
  margin-bottom: 18px;
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.22);
}
.hero {
  position: relative;
  overflow: hidden;
}
.hero::after {
  content: "";
  position: absolute;
  inset: auto -40px -60px auto;
  width: 180px;
  height: 180px;
  border-radius: 999px;
  background: radial-gradient(circle, rgba(89, 213, 176, 0.18), transparent 70%);
  pointer-events: none;
}
.eyebrow {
  margin: 0 0 8px;
  color: var(--accent);
  text-transform: uppercase;
  letter-spacing: 0.16em;
  font-size: 12px;
}
h1 {
  margin: 0;
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: clamp(28px, 4vw, 44px);
  line-height: 1;
}
h2 {
  margin: 0 0 16px;
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 18px;
}
.subtitle {
  margin: 14px 0 0;
  color: var(--muted);
  line-height: 1.6;
}
.badges {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 16px;
}
.badge {
  border: 1px solid rgba(89, 213, 176, 0.24);
  color: var(--accent);
  border-radius: 999px;
  padding: 6px 10px;
  font-size: 12px;
  letter-spacing: 0.04em;
}
.note, .inline-status {
  color: var(--muted);
  line-height: 1.5;
  font-size: 14px;
}
code {
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 13px;
  color: #bef4e4;
  background: rgba(11, 17, 21, 0.9);
  padding: 3px 6px;
  border-radius: 8px;
}
.field {
  margin-bottom: 16px;
}
.field:last-child {
  margin-bottom: 0;
}
label {
  display: block;
  color: var(--muted);
  font-size: 13px;
  margin-bottom: 6px;
  letter-spacing: 0.02em;
}
input, textarea {
  width: 100%;
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(11, 17, 21, 0.92);
  color: var(--text);
  font: inherit;
  font-size: 14px;
  padding: 12px 14px;
  transition: border-color 0.15s;
}
input:focus, textarea:focus {
  outline: none;
  border-color: rgba(89, 213, 176, 0.5);
}
textarea {
  resize: vertical;
  min-height: 60px;
}
button {
  border-radius: 12px;
  border: 1px solid var(--line);
  background: rgba(11, 17, 21, 0.92);
  color: var(--text);
  font: inherit;
  padding: 10px 18px;
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s;
}
button:hover { border-color: #3d5b6c; }
button.primary {
  background: linear-gradient(180deg, #1d7c63, #176551);
  border-color: rgba(89, 213, 176, 0.26);
}
button.primary:hover {
  background: linear-gradient(180deg, #22906f, #1a7a5e);
}
.actions-row {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}
.preview {
  min-height: 120px;
  max-height: 400px;
  overflow: auto;
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: rgba(8, 12, 15, 0.95);
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 12px;
  white-space: pre-wrap;
  color: #bef4e4;
  margin-top: 12px;
}
.log {
  min-height: 120px;
  max-height: 200px;
  overflow: auto;
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--line);
  background: rgba(8, 12, 15, 0.95);
  font-family: "Iosevka Etoile", "IBM Plex Mono", monospace;
  font-size: 13px;
  white-space: pre-wrap;
}
.entry { display: block; }
.time { color: #5f7583; margin-right: 6px; }
a { color: #8be3cb; }
.saved { color: var(--accent); }
.error { color: var(--danger); }

@media (max-width: 640px) {
  .page { padding: 18px 14px 32px; }
  .card { padding: 16px; border-radius: 16px; }
}
"##;

const JS: &str = r##"
(function() {

const CONFIG_TRAIT = 'www.seo';

// SEO field definitions: key → { selector for current head tag, meta attr }
const FIELDS = {
  title:               { tag: 'title',                                     attr: null },
  description:         { tag: 'meta[name="description"]',                  attr: 'content' },
  keywords:            { tag: 'meta[name="keywords"]',                     attr: 'content' },
  author:              { tag: 'meta[name="author"]',                       attr: 'content' },
  canonical:           { tag: 'link[rel="canonical"]',                     attr: 'href' },
  og_title:            { tag: 'meta[property="og:title"]',                 attr: 'content' },
  og_description:      { tag: 'meta[property="og:description"]',           attr: 'content' },
  og_site_name:        { tag: 'meta[property="og:site_name"]',             attr: 'content' },
  og_image:            { tag: 'meta[property="og:image"]',                 attr: 'content' },
  twitter_title:       { tag: 'meta[name="twitter:title"]',                attr: 'content' },
  twitter_description: { tag: 'meta[name="twitter:description"]',          attr: 'content' },
  twitter_image:       { tag: 'meta[name="twitter:image"]',                attr: 'content' },
  jsonld_name:         { jsonld: 'name' },
  jsonld_description:  { jsonld: 'description' },
  jsonld_author:       { jsonld: 'author.name' },
};

function esc(s) {
  const d = document.createElement('div');
  d.textContent = String(s == null ? '' : s);
  return d.innerHTML;
}

function log(msg, cls) {
  const el = document.getElementById('activityLog');
  if (!el) return;
  const t = new Date().toTimeString().slice(0, 8);
  el.innerHTML += '<span class="entry"><span class="time">[' + t + ']</span> <span' + (cls ? ' class="'+cls+'"' : '') + '>' + esc(msg) + '</span></span>';
  el.scrollTop = el.scrollHeight;
}

function status(msg, isError) {
  const el = document.getElementById('status');
  el.textContent = msg;
  el.style.color = isError ? '#ef6b73' : '#9ab0bf';
  if (msg) setTimeout(() => { if (el.textContent === msg) el.textContent = ''; }, 5000);
}

async function callConfig(action, key, value) {
  if (!window._traitsSDK) return { ok: false, error: 'SDK not loaded' };
  const args = [action, CONFIG_TRAIT];
  if (key) args.push(key);
  if (value !== undefined) args.push(value);
  return window._traitsSDK.call('sys.config', args);
}

// ── Read current <head> values as defaults ──
function readCurrentHead() {
  const vals = {};
  for (const [key, def] of Object.entries(FIELDS)) {
    if (def.jsonld) continue;
    if (key === 'title') {
      // Read from parent document's <head>
      vals[key] = document.title || '';
    } else if (def.tag && def.attr) {
      // Look in the parent document head (SPA shell)
      const topDoc = window.top ? window.top.document : document;
      const el = topDoc.querySelector(def.tag);
      vals[key] = el ? (el.getAttribute(def.attr) || '') : '';
    }
  }
  // JSON-LD
  try {
    const topDoc = window.top ? window.top.document : document;
    const ldEl = topDoc.querySelector('script[type="application/ld+json"]');
    if (ldEl) {
      const ld = JSON.parse(ldEl.textContent);
      vals.jsonld_name = ld.name || '';
      vals.jsonld_description = ld.description || '';
      vals.jsonld_author = (ld.author && ld.author.name) || '';
    }
  } catch(_) {}
  return vals;
}

// ── Fill form fields ──
function fillForm(vals) {
  document.querySelectorAll('[data-key]').forEach(el => {
    const key = el.dataset.key;
    if (vals[key] !== undefined && vals[key] !== '') {
      el.value = vals[key];
    }
  });
}

// ── Collect form values ──
function collectForm() {
  const vals = {};
  document.querySelectorAll('[data-key]').forEach(el => {
    vals[el.dataset.key] = el.value;
  });
  return vals;
}

// ── Load from sys.config ──
async function loadConfig() {
  const badge = document.getElementById('connBadge');
  const res = await callConfig('list');
  if (!res.ok) {
    badge.textContent = 'no server';
    badge.style.borderColor = 'rgba(239, 107, 115, 0.4)';
    badge.style.color = '#ef6b73';
    log('No server/helper connected — showing current page defaults', 'error');
    // Fall back to reading current <head>
    fillForm(readCurrentHead());
    updatePreview();
    return;
  }

  badge.textContent = 'connected';
  badge.style.borderColor = 'rgba(89, 213, 176, 0.4)';

  const config = res.result?.config || [];
  const vals = readCurrentHead(); // Start with defaults

  for (const entry of config) {
    if (entry.key && entry.value !== undefined && entry.value !== '') {
      vals[entry.key] = entry.value;
    }
  }

  fillForm(vals);
  log('Loaded ' + config.length + ' config entries from sys.config');
  updatePreview();
}

// ── Save all fields to sys.config ──
async function saveAll() {
  const vals = collectForm();
  const btn = document.getElementById('btnSave');
  btn.disabled = true;
  btn.textContent = 'Saving...';

  let saved = 0;
  let errors = 0;

  for (const [key, value] of Object.entries(vals)) {
    if (!value.trim()) continue; // Skip empty fields
    const res = await callConfig('set', key, value);
    if (res.ok) {
      saved++;
    } else {
      errors++;
      log('Failed to save ' + key + ': ' + (res.error || 'unknown'), 'error');
    }
  }

  btn.disabled = false;
  btn.textContent = 'Save All';

  if (errors > 0) {
    status('Saved ' + saved + ', failed ' + errors, true);
  } else {
    status('Saved ' + saved + ' settings');
    log('Saved ' + saved + ' settings to sys.config', 'saved');
  }

  updatePreview();
}

// ── Apply values to live page <head> ──
function applyNow() {
  const vals = collectForm();
  const topDoc = window.top ? window.top.document : document;

  for (const [key, def] of Object.entries(FIELDS)) {
    const value = vals[key];
    if (!value) continue;

    if (key === 'title') {
      topDoc.title = value;
      continue;
    }

    if (def.jsonld) continue; // JSON-LD handled separately below

    if (def.tag && def.attr) {
      let el = topDoc.querySelector(def.tag);
      if (el) {
        el.setAttribute(def.attr, value);
      } else {
        // Create if missing (e.g., og:image, twitter:image)
        el = topDoc.createElement(def.tag.startsWith('link') ? 'link' : 'meta');
        if (def.tag.includes('property=')) {
          const prop = def.tag.match(/property="([^"]+)"/);
          if (prop) el.setAttribute('property', prop[1]);
        } else if (def.tag.includes('name=')) {
          const name = def.tag.match(/name="([^"]+)"/);
          if (name) el.setAttribute('name', name[1]);
        }
        el.setAttribute(def.attr, value);
        topDoc.head.appendChild(el);
      }
    }
  }

  // Update JSON-LD
  try {
    const ldEl = topDoc.querySelector('script[type="application/ld+json"]');
    if (ldEl) {
      const ld = JSON.parse(ldEl.textContent);
      if (vals.jsonld_name) ld.name = vals.jsonld_name;
      if (vals.jsonld_description) ld.description = vals.jsonld_description;
      if (vals.jsonld_author) {
        if (!ld.author) ld.author = { "@type": "Person" };
        ld.author.name = vals.jsonld_author;
      }
      ldEl.textContent = JSON.stringify(ld, null, 2);
    }
  } catch(_) {}

  status('Applied to live page');
  log('Applied SEO tags to current page <head>', 'saved');
}

// ── Reset form to current <head> defaults ──
async function resetAll() {
  // Delete all config keys
  const res = await callConfig('list');
  if (res.ok && res.result?.config) {
    for (const entry of res.result.config) {
      await callConfig('delete', entry.key);
    }
    log('Cleared ' + res.result.config.length + ' config entries', 'saved');
  }

  fillForm(readCurrentHead());
  status('Reset to defaults');
  updatePreview();
}

// ── Generate preview ──
function updatePreview() {
  const vals = collectForm();
  const lines = [];

  if (vals.title)
    lines.push('<title>' + esc(vals.title) + '</title>');
  if (vals.description)
    lines.push('<meta name="description" content="' + esc(vals.description) + '">');
  if (vals.keywords)
    lines.push('<meta name="keywords" content="' + esc(vals.keywords) + '">');
  if (vals.author)
    lines.push('<meta name="author" content="' + esc(vals.author) + '">');
  if (vals.canonical)
    lines.push('<link rel="canonical" href="' + esc(vals.canonical) + '">');

  lines.push('');
  lines.push('<!-- Open Graph -->');
  lines.push('<meta property="og:type" content="website">');
  if (vals.canonical)
    lines.push('<meta property="og:url" content="' + esc(vals.canonical) + '">');
  if (vals.og_title)
    lines.push('<meta property="og:title" content="' + esc(vals.og_title) + '">');
  if (vals.og_description)
    lines.push('<meta property="og:description" content="' + esc(vals.og_description) + '">');
  if (vals.og_site_name)
    lines.push('<meta property="og:site_name" content="' + esc(vals.og_site_name) + '">');
  if (vals.og_image)
    lines.push('<meta property="og:image" content="' + esc(vals.og_image) + '">');

  lines.push('');
  lines.push('<!-- Twitter Card -->');
  lines.push('<meta name="twitter:card" content="summary">');
  if (vals.twitter_title)
    lines.push('<meta name="twitter:title" content="' + esc(vals.twitter_title) + '">');
  if (vals.twitter_description)
    lines.push('<meta name="twitter:description" content="' + esc(vals.twitter_description) + '">');
  if (vals.twitter_image)
    lines.push('<meta name="twitter:image" content="' + esc(vals.twitter_image) + '">');

  document.getElementById('preview').textContent = lines.join('\n');
}

// ── Init ──
loadConfig();

// Update preview on any field change
document.querySelectorAll('[data-key]').forEach(el => {
  el.addEventListener('input', updatePreview);
});

// Expose to onclick
window.saveAll = saveAll;
window.applyNow = applyNow;
window.resetAll = resetAll;

})();
"##;
