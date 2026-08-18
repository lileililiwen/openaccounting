// Keyboard shortcuts (`u1-keyboard-shortcuts`).
//
// Two-step shortcuts use the vim convention: press `g` first,
// then within 1s press the second key. Single keys fire
// immediately. Inputs and textareas swallow the keys so the
// shortcut never fires while the user is typing. Auth pages
// (`/login`, `/register`) opt out by including
// `<body data-shortcuts-off>`.
//
// Bindings:
//
//   g l    → Ledgers
//   g t    → Transactions (of the current ledger if any)
//   g a    → Accounts
//   g r    → Reports
//   c      → Create a transaction (only on the list page)
//   ?      → Open the help overlay
//   Esc    → Close the help overlay

(function () {
  if (document.body && document.body.dataset.shortcutsOff === 'true') {
    return;
  }
  var STATE = { prefix: null, prefixTimer: null };
  var TIMEOUT_MS = 1000;

  function isTypingTarget(el) {
    if (!el) return false;
    var tag = (el.tagName || '').toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select') return true;
    if (el.isContentEditable) return true;
    return false;
  }

  function withPrefix(key) {
    if (STATE.prefixTimer) clearTimeout(STATE.prefixTimer);
    STATE.prefix = key;
    STATE.prefixTimer = setTimeout(function () {
      STATE.prefix = null;
      STATE.prefixTimer = null;
    }, TIMEOUT_MS);
  }

  function navigate(path) {
    // Replace `:id` with the current ledger id when present.
    var m = window.location.pathname.match(/^\/ledgers\/([0-9a-fA-F-]+)/);
    if (m && path.indexOf(':id') !== -1) {
      path = path.replace(':id', m[1]);
    }
    window.location.href = path;
  }

  function isOnListPage() {
    return /^\/ledgers\/[0-9a-fA-F-]+\/transactions\/?$/.test(
      window.location.pathname
    );
  }

  function showHelp() {
    var overlay = document.getElementById('shortcuts-help');
    if (!overlay) return;
    overlay.classList.remove('hidden');
    overlay.setAttribute('aria-hidden', 'false');
  }
  function hideHelp() {
    var overlay = document.getElementById('shortcuts-help');
    if (!overlay) return;
    overlay.classList.add('hidden');
    overlay.setAttribute('aria-hidden', 'true');
  }

  // Global click-to-dismiss for the help overlay backdrop.
  document.addEventListener('click', function (e) {
    var overlay = document.getElementById('shortcuts-help');
    if (!overlay || overlay.classList.contains('hidden')) return;
    if (e.target && e.target.id === 'shortcuts-help-backdrop') {
      hideHelp();
    }
  });

  document.addEventListener('keydown', function (e) {
    if (isTypingTarget(e.target)) return;
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    var key = e.key || '';

    // Escape closes the overlay unconditionally.
    if (key === 'Escape') {
      if (document.getElementById('shortcuts-help') &&
          !document.getElementById('shortcuts-help').classList.contains('hidden')) {
        hideHelp();
        e.preventDefault();
        return;
      }
    }

    // The `?` key requires Shift on US layouts; some keyboards
    // send it without Shift, so we also accept the literal '?'.
    if (key === '?' || (key === '/' && e.shiftKey)) {
      showHelp();
      e.preventDefault();
      return;
    }

    if (STATE.prefix === 'g') {
      if (key === 'l') {
        e.preventDefault();
        STATE.prefix = null;
        navigate('/ledgers');
        return;
      }
      if (key === 't') {
        e.preventDefault();
        STATE.prefix = null;
        navigate('/ledgers/:id/transactions');
        return;
      }
      if (key === 'a') {
        e.preventDefault();
        STATE.prefix = null;
        navigate('/ledgers/:id/accounts');
        return;
      }
      if (key === 'r') {
        e.preventDefault();
        STATE.prefix = null;
        navigate('/ledgers/:id/reports');
        return;
      }
      // Unrecognised second key — drop the prefix.
      STATE.prefix = null;
      return;
    }

    if (key === 'g') {
      e.preventDefault();
      withPrefix('g');
      return;
    }

    if (key === 'c' && isOnListPage()) {
      e.preventDefault();
      navigate('/ledgers/:id/transactions/new');
      return;
    }
  });
})();