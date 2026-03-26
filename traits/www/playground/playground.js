var allTraits = [];
var selectedTrait = null;
var dropdownIdx = -1;

function normalizeTraits(payload) {
  if (Array.isArray(payload)) return payload;
  if (payload && Array.isArray(payload.traits)) return payload.traits;
  if (payload && Array.isArray(payload.result)) return payload.result;
  return [];
}

function initPlayground(traits) {
  allTraits = normalizeTraits(traits).sort(function(a, b) {
    return (a.path || '').localeCompare(b.path || '');
  });
  // If URL has ?trait=xxx, auto-select
  var p = new URLSearchParams(window.location.search);
  if (p.get('trait')) {
    var t = allTraits.find(function(t) { return t.path === p.get('trait'); });
    if (t) selectTrait(t);
    document.getElementById('traitSearch').value = p.get('trait');
  }
}

async function bootstrapPlayground() {
  try {
    const traits = await window._traitsSDK.list();
    initPlayground(traits);
  } catch (error) {
    document.getElementById('resultPanel').hidden = false;
    document.getElementById('resultOutput').innerHTML = '<span class="err">' + esc(error.message || String(error)) + '</span>';
  }
}

// ── Search + Dropdown ──
const searchEl = document.getElementById('traitSearch');
const dropdown = document.getElementById('traitDropdown');

searchEl.addEventListener('input', function() {
  const q = this.value.toLowerCase().trim();
  if (!q) { dropdown.classList.remove('open'); return; }
  const matches = allTraits.filter(t =>
    (t.path || '').toLowerCase().includes(q) ||
    (t.description || '').toLowerCase().includes(q)
  ).slice(0, 20);
  if (matches.length === 0) { dropdown.classList.remove('open'); return; }
  dropdown.innerHTML = matches.map((t, i) =>
    '<div class="item' + (i === 0 ? ' active' : '') + '" data-idx="' + i + '">' +
    '<span class="path">' + esc(t.path) + '</span>' +
    '<span class="ddesc">' + esc(t.description || '') + '</span></div>'
  ).join('');
  dropdown.classList.add('open');
  dropdownIdx = 0;

  dropdown.querySelectorAll('.item').forEach(function(el) {
    el.addEventListener('click', function() {
      const idx = parseInt(this.dataset.idx);
      selectTrait(matches[idx]);
      searchEl.value = matches[idx].path;
      dropdown.classList.remove('open');
    });
  });
});

searchEl.addEventListener('keydown', function(e) {
  const items = dropdown.querySelectorAll('.item');
  if (!items.length) return;
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    dropdownIdx = Math.min(dropdownIdx + 1, items.length - 1);
    items.forEach((el, i) => el.classList.toggle('active', i === dropdownIdx));
    items[dropdownIdx].scrollIntoView({ block: 'nearest' });
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    dropdownIdx = Math.max(dropdownIdx - 1, 0);
    items.forEach((el, i) => el.classList.toggle('active', i === dropdownIdx));
    items[dropdownIdx].scrollIntoView({ block: 'nearest' });
  } else if (e.key === 'Enter') {
    e.preventDefault();
    if (dropdownIdx >= 0) items[dropdownIdx].click();
  } else if (e.key === 'Escape') {
    dropdown.classList.remove('open');
  }
});

document.addEventListener('click', function(e) {
  if (!e.target.closest('.selector')) dropdown.classList.remove('open');
});

// ── Trait Selection ──
function selectTrait(t) {
  selectedTrait = t;
  document.getElementById('traitName').textContent = t.path;
  document.getElementById('traitDesc').textContent = t.description || '';
  document.getElementById('traitPanel').hidden = false;
  document.getElementById('resultPanel').hidden = true;
  document.getElementById('elapsed').textContent = '';

  // Update URL without reload
  const url = new URL(window.location);
  url.searchParams.set('trait', t.path);
  history.replaceState(null, '', url);

  // Build param form
  const form = document.getElementById('paramsForm');
  const params = t.params || [];
  if (params.length === 0) {
    form.innerHTML = '<div class="no-params">No parameters — just hit Run</div>';
    return;
  }
  form.innerHTML = params.map(function(p, i) {
    const req = p.required !== false && !p.optional;
    const badge = req ? '<span class="req">required</span>' : '<span class="opt">optional</span>';
    const typeStr = p.type || 'string';
    const desc = p.description || '';
    const examples = (p.examples || []).map(function(ex) {
      return '<span class="chip" data-param="' + i + '" data-val="' + esc(String(ex)) + '">' + esc(String(ex)) + '</span>';
    }).join('');

    // Choose input type based on param type
    var input;
    if (typeStr === 'bool') {
      input = '<select data-param="' + i + '"><option value="true">true</option><option value="false">false</option></select>';
    } else if (desc.length > 100 || typeStr.startsWith('list') || typeStr.startsWith('map') || typeStr === 'object') {
      input = '<textarea data-param="' + i + '" rows="3" placeholder="' + esc(typeStr) + '"></textarea>';
    } else {
      input = '<input data-param="' + i + '" type="text" placeholder="' + esc(typeStr) + '" />';
    }
    return '<div class="param">' +
      '<div class="param-label"><span class="name">' + esc(p.name) + '</span><span class="type">' + esc(typeStr) + '</span>' + badge + '</div>' +
      (desc ? '<div class="param-desc">' + esc(desc) + '</div>' : '') +
      input +
      (examples ? '<div class="examples">' + examples + '</div>' : '') +
      '</div>';
  }).join('');

  // Wire up example chips
  form.querySelectorAll('.chip').forEach(function(chip) {
    chip.addEventListener('click', function() {
      const idx = this.dataset.param;
      const val = this.dataset.val;
      const inp = form.querySelector('[data-param="' + idx + '"]');
      if (inp) { inp.value = val; inp.focus(); }
    });
  });
}

// ── Run ──
document.getElementById('btnRun').addEventListener('click', runTrait);
document.addEventListener('keydown', function(e) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && selectedTrait) runTrait();
});

async function runTrait() {
  if (!selectedTrait) return;
  const btn = document.getElementById('btnRun');
  const elapsedEl = document.getElementById('elapsed');
  btn.disabled = true;
  btn.textContent = 'Running...';
  elapsedEl.textContent = '';

  // Collect args
  const params = selectedTrait.params || [];
  var args = [];
  params.forEach(function(p, i) {
    const inp = document.querySelector('#paramsForm [data-param="' + i + '"]');
    var val = inp ? inp.value : '';
    // Try to parse JSON for non-string types
    if (val && p.type !== 'string') {
      try { val = JSON.parse(val); } catch(e) { /* leave as string */ }
    }
    args.push(val === '' ? null : val);
  });
  // Trim trailing nulls for optional params
  while (args.length > 0 && args[args.length - 1] === null) args.pop();

  const start = Date.now();
  const timer = setInterval(function() {
    elapsedEl.textContent = ((Date.now() - start) / 1000).toFixed(1) + 's';
  }, 100);

  try {
    const res = await window._traitsSDK.call(selectedTrait.path, args);
    clearInterval(timer);
    const elapsed = ((Date.now() - start) / 1000).toFixed(2);
    elapsedEl.textContent = elapsed + 's (' + (res.dispatch || '?') + ')';

    const resultEl = document.getElementById('resultOutput');
    document.getElementById('resultPanel').hidden = false;

    if (!res.ok) {
      resultEl.innerHTML = '<span class="err">' + esc(res.error) + '</span>';
    } else {
      const val = res.result;
      resultEl.textContent = typeof val === 'string' ? val : JSON.stringify(val, null, 2);
    }
    resultEl.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  } catch(e) {
    clearInterval(timer);
    elapsedEl.textContent = 'error';
    document.getElementById('resultPanel').hidden = false;
    document.getElementById('resultOutput').innerHTML = '<span class="err">' + esc(e.message) + '</span>';
  }
  btn.disabled = false;
  btn.textContent = 'Run';
}

// ── Copy ──
document.getElementById('btnCopy').addEventListener('click', function() {
  const text = document.getElementById('resultOutput').textContent;
  navigator.clipboard.writeText(text).then(function() {
    const btn = document.getElementById('btnCopy');
    btn.textContent = 'Copied!';
    setTimeout(function() { btn.textContent = 'Copy'; }, 1500);
  });
});

function esc(s) { const d = document.createElement('div'); d.textContent = s; return d.innerHTML; }

bootstrapPlayground();
