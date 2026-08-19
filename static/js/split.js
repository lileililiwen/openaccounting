// split.js — auto-balancing split composer
// (`a4-split-transaction-ux`).
//
// Adds a "Split" button next to the existing "+ Add line"
// button. Clicking it inserts N-1 empty rows, then computes
// the auto-balance for the last row: the amount needed to
// make total debits equal total credits, and the opposite
// direction.
//
// No build step. Loaded via a plain <script src> tag.

(function () {
  function parseAmount(s) {
    var n = parseFloat(s);
    return isFinite(n) ? n : 0;
  }

  function setField(row, selector, value) {
    var el = row.querySelector(selector);
    if (el) el.value = value;
  }

  function rewriteIndices(row, idx) {
    row.querySelectorAll('input, select').forEach(function (el) {
      var name = el.getAttribute('name');
      if (name) {
        el.setAttribute(
          'name',
          name.replace(/lines\[\d+\]/, 'lines[' + idx + ']')
        );
      }
    });
  }

  function recomputeBalance(container) {
    var debits = 0;
    var credits = 0;
    container.querySelectorAll('.posting').forEach(function (row) {
      var dir = row.querySelector('select[name$="[direction]"]');
      var amt = row.querySelector('input[name$="[amount]"]');
      if (!dir || !amt) return;
      var v = parseAmount(amt.value);
      if (dir.value === 'DEBIT') debits += v;
      else if (dir.value === 'CREDIT') credits += v;
    });
    var net = debits - credits;
    var display = document.getElementById('split-balance');
    if (!display) return;
    // `ux-transaction-entry`: don't claim "balanced" before the
    // user has entered any amount.
    if (debits + credits === 0) {
      display.textContent = '—';
      display.classList.remove('text-emerald-600', 'text-rose-600');
      return;
    }
    display.textContent = (net === 0 ? '✓ balanced' : 'net ' + net.toFixed(2)).toString();
    display.classList.toggle('text-emerald-600', net === 0);
    display.classList.toggle('text-rose-600', net !== 0);
  }

  function init() {
    var container = document.getElementById('postings');
    if (!container) return;
    var addBtn = document.getElementById('add-line');
    var splitBtn = document.getElementById('split-btn');
    var splitN = document.getElementById('split-n');
    if (!splitBtn) return;

    // Track the next index. New rows append at the end and
    // increment.
    var idx = container.querySelectorAll('.posting').length;

    function appendBlankRow() {
      var last = container.querySelector('.posting');
      if (!last) return null;
      var clone = last.cloneNode(true);
      rewriteIndices(clone, idx);
      clone.querySelectorAll('input').forEach(function (el) {
        el.value = '';
      });
      clone.querySelectorAll('select').forEach(function (el) {
        el.selectedIndex = 0;
      });
      container.appendChild(clone);
      idx++;
      return clone;
    }

    splitBtn.addEventListener('click', function () {
      var n = parseInt(splitN && splitN.value, 10) || 2;
      if (n < 2) n = 2;
      if (n > 50) n = 50;
      // Insert (n - rows) blank rows so the total row count is n.
      var existing = container.querySelectorAll('.posting').length;
      for (var i = existing; i < n; i++) {
        appendBlankRow();
      }
      // Mark the LAST row as auto-balance: clear its
      // amount + direction; the user picks the account.
      var rows = container.querySelectorAll('.posting');
      var lastRow = rows[rows.length - 1];
      if (lastRow) {
        setField(lastRow, 'input[name$="[amount]"]', '');
        setField(lastRow, 'select[name$="[direction]"]', '');
      }
      recomputeBalance(container);
    });

    // Listen for changes to recompute the live balance
    // display.
    container.addEventListener('input', function (ev) {
      if (ev.target.matches('input, select')) {
        recomputeBalance(container);
      }
    });
    container.addEventListener('change', function (ev) {
      if (ev.target.matches('select')) {
        recomputeBalance(container);
      }
    });

    // If the user edits the amount in the last row, fill in
    // the auto-balance. This is the "compute on the fly"
    // half of the spec — leave the others for the user.
    container.addEventListener('input', function (ev) {
      if (!ev.target.matches('input[name$="[amount]"]')) return;
      var rows = Array.from(container.querySelectorAll('.posting'));
      var last = rows[rows.length - 1];
      if (!last || ev.target !== last.querySelector('input[name$="[amount]"]')) return;
      // Sum the OTHER rows; the last must offset them.
      var debits = 0;
      var credits = 0;
      rows.slice(0, -1).forEach(function (row) {
        var dir = row.querySelector('select[name$="[direction]"]');
        var amt = row.querySelector('input[name$="[amount]"]');
        if (!dir || !amt) return;
        var v = parseAmount(amt.value);
        if (dir.value === 'DEBIT') debits += v;
        else if (dir.value === 'CREDIT') credits += v;
      });
      var lastAmt = parseFloat(ev.target.value);
      if (!isFinite(lastAmt)) return;
      // After inserting the last amount, net must be zero.
      var curDir = last.querySelector('select[name$="[direction]"]');
      var curDirVal = curDir ? curDir.value : '';
      var lastSign = curDirVal === 'CREDIT' ? -1 : 1;
      var running = debits - credits + lastSign * lastAmt;
      if (running === 0) {
        recomputeBalance(container);
        return;
      }
      // User-typed value didn't balance; auto-correct by
      // toggling direction so the net is zero.
      if (curDir) {
        curDir.value = curDirVal === 'DEBIT' ? 'CREDIT' : 'DEBIT';
      }
      recomputeBalance(container);
    });

    recomputeBalance(container);

    // The base template's "+ Add line" listener is inline;
    // we rely on its `idx` counter to stay consistent. After
    // any add, recount so our own indices are correct.
    if (addBtn) {
      addBtn.addEventListener('click', function () {
        setTimeout(function () {
          idx = container.querySelectorAll('.posting').length;
          recomputeBalance(container);
        }, 0);
      });
    }

    // `ux-transaction-entry`: block meaningless same-account
    // entries and confirm far-future dates before submitting.
    var form = document.getElementById('txn-form');
    if (form) {
      form.addEventListener('submit', function (ev) {
        var rows = Array.from(container.querySelectorAll('.posting'));
        var byAccount = {};
        rows.forEach(function (row) {
          var acc = row.querySelector('select[name$="[account_id]"]');
          var dir = row.querySelector('select[name$="[direction]"]');
          if (!acc || !acc.value || !dir) return;
          if (!byAccount[acc.value]) byAccount[acc.value] = { d: false, c: false };
          if (dir.value === 'DEBIT') byAccount[acc.value].d = true;
          else if (dir.value === 'CREDIT') byAccount[acc.value].c = true;
        });
        var badId = null;
        Object.keys(byAccount).forEach(function (id) {
          if (byAccount[id].d && byAccount[id].c) badId = id;
        });
        if (badId) {
          var opt = container.querySelector('option[value="' + badId + '"]');
          var label = opt ? opt.textContent : 'this account';
          ev.preventDefault();
          window.alert(
            'This entry moves money within "' + label +
            '" (the same account on both sides) — pick a different account for one of the lines.'
          );
          return;
        }
        var dateEl = form.querySelector('input[name="date"]');
        if (dateEl && dateEl.value) {
          var d = new Date(dateEl.value + 'T00:00:00');
          var today = new Date();
          today.setHours(0, 0, 0, 0);
          var days = Math.round((d - today) / 86400000);
          if (days > 30) {
            var ok = window.confirm(
              'You are recording a transaction dated ' + dateEl.value +
              ' (' + days + ' days in the future) — is that intentional?'
            );
            if (!ok) {
              ev.preventDefault();
            }
          }
        }
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();