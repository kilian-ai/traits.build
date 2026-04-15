// Shared terminal surface for traits.build and sibling SPAs.
// Keep this file dependency-free so it can be concatenated into classic scripts.
(function() {
  if (typeof window === 'undefined') return;

  const defaults = {
    sentinels: {
      clear: '\\x1b[CLEAR]',
      restOpen: '\\x1b[REST]',
      restClose: '\\x1b[/REST]',
      webllmOpen: '\\x1b[WEBLLM]',
      webllmClose: '\\x1b[/WEBLLM]',
      voiceOpen: '\\x1b[VOICE]',
      voiceClose: '\\x1b[/VOICE]'
    },
    prompt: '\\x1b[32mtraits \\x1b[0m',
    storageKeys: {
      scrollback: 'traits.terminal.scrollback',
      history: 'traits.terminal.history',
      pvfs: 'traits.pvfs',
      legacyVfs: 'traits.terminal.vfs'
    }
  };

  window.TerminalShared = window.TerminalShared || {};
  window.TerminalShared.defaults = defaults;
})();
