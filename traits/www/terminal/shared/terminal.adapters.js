// Shared terminal adapters: transport + persistence.
// Host repos can override/extend these adapters while keeping terminal core stable.
(function() {
  if (typeof window === 'undefined') return;

  function createPersistenceAdapter(opts) {
    const keys = opts && opts.keys ? opts.keys : {};
    const getBackgroundCall = opts && opts.getBackgroundCall ? opts.getBackgroundCall : () => null;
    const serializeAddon = opts && opts.serializeAddon ? opts.serializeAddon : null;

    const saveState = () => {
      if (serializeAddon) {
        try { localStorage.setItem(keys.scrollback, serializeAddon.serialize()); } catch (_) {}
      }
      const backgroundCall = getBackgroundCall();
      if (!backgroundCall) return;

      backgroundCall('cli_get_history').then(res => {
        if (res && res.ok && typeof res.result === 'string') {
          try { localStorage.setItem(keys.history, res.result); } catch (_) {}
        }
      }).catch(() => {});

      backgroundCall('pvfs_dump').then(res => {
        if (res && res.ok && typeof res.result === 'string') {
          try {
            localStorage.setItem(keys.pvfs, res.result);
            localStorage.setItem('traits.pvfs.ts', String(Date.now()));
            if (keys.legacyVfs) localStorage.setItem(keys.legacyVfs, res.result);
          } catch (_) {}
        }
      }).catch(() => {});
    };

    const attachAutoSaveHandlers = () => {
      window.addEventListener('pagehide', saveState);
      window.addEventListener('hashchange', saveState);
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') saveState();
      });
    };

    const restoreSession = async () => {
      const backgroundCall = getBackgroundCall();
      if (!backgroundCall) return;

      const savedHistory = localStorage.getItem(keys.history);
      if (savedHistory) {
        try { await backgroundCall('cli_set_history', { history_json: savedHistory }); } catch (_) {}
      }

      let savedVfs = localStorage.getItem(keys.pvfs);
      if (!savedVfs && keys.legacyVfs) {
        savedVfs = localStorage.getItem(keys.legacyVfs);
        if (savedVfs) {
          try { localStorage.setItem(keys.pvfs, savedVfs); } catch (_) {}
        }
      }
      if (savedVfs) {
        try { await backgroundCall('pvfs_load', { json: savedVfs }); } catch (_) {}
      }
    };

    return { saveState, attachAutoSaveHandlers, restoreSession };
  }

  async function initTraitsTransport(ctx) {
    let activeSdk = ctx.activeSdk || (window._traitsSDK || null);
    let backgroundCall = null;
    let wasm = null;

    if (activeSdk && typeof activeSdk.backgroundCall === 'function') {
      await activeSdk.initWorkerPool();
      backgroundCall = (cmd, payload = {}) => activeSdk.backgroundCall(cmd, payload);
      const status = activeSdk.status || {};
      if (ctx.setStatus) ctx.setStatus('WASM worker', 'ready');
      if (window.TraitsWasm && window.TraitsWasm.register_task) {
        try { window.TraitsWasm.register_task('terminal', 'Terminal', 'service', Date.now(), 'xterm.js CLI session'); } catch (_) {}
      }
      if (ctx.onReady) ctx.onReady({ wasm: null, traitCount: status.traits || 0, wasmCount: status.callable || 0, background: true });
      return { activeSdk, backgroundCall, wasm };
    }

    if (window.TraitsWasm && window.TraitsWasm.cli_input) {
      wasm = window.TraitsWasm;
      const count = wasm.is_registered ? JSON.parse(wasm.callable_traits()).length : 0;
      if (ctx.setStatus) ctx.setStatus('WASM (SPA)', 'ready');
      if (ctx.onReady) ctx.onReady({ wasm, traitCount: 0, wasmCount: count, background: false });
    } else {
      const wasmJsUrl = '/wasm/traits_wasm.js';
      const wasmBinUrl = '/wasm/traits_wasm_bg.wasm';
      const mod = await import(wasmJsUrl);
      await mod.default(wasmBinUrl);
      const initResult = JSON.parse(mod.init());
      wasm = mod;
      const count = initResult.traits_registered || 0;
      const wasmCount = initResult.wasm_callable || 0;
      if (ctx.setStatus) ctx.setStatus(`${count} traits (${wasmCount} WASM)`, 'ready');
      if (ctx.onReady) ctx.onReady({ wasm, traitCount: count, wasmCount, background: false });
    }

    if (window.Traits) {
      activeSdk = new window.Traits({ useWasm: false, useHelper: false, server: '' });
      activeSdk.attachWasm(wasm);
      activeSdk.setBackgroundBinding('sdk.background.direct');
      backgroundCall = (cmd, payload = {}) => activeSdk.backgroundCall(cmd, payload, { impl: 'sdk.background.direct' });
    }

    if (wasm && wasm.register_task) {
      try { wasm.register_task('terminal', 'Terminal', 'service', Date.now(), 'xterm.js CLI session'); } catch (_) {}
    }

    return { activeSdk, backgroundCall, wasm };
  }

  window.TerminalSharedAdapters = window.TerminalSharedAdapters || {};
  window.TerminalSharedAdapters.createPersistenceAdapter = createPersistenceAdapter;
  window.TerminalSharedAdapters.initTraitsTransport = initTraitsTransport;
})();
