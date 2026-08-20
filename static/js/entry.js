// entry.js — balancing-line transaction editor
// (`a12-transaction-entry-ease`).
//
// Replaces split.js. One posting line is always the *balancing
// line*: its amount is computed live so Σ debits = Σ credits, and
// its direction flips to the opposite side of the net. Editing any
// other line recomputes it. Typing into the balancer promotes it to
// a fixed line and designates the next empty line (or a new one) as
// the balancer. A status readout next to the save buttons reports
// "balanced — ready to save" or the exact shortfall.
//
// No build step. Loaded via a plain <script src> tag.

(function () {
  'use strict';

  var container = document.getElementById('postings');
  if (!container) return;
  var form = document.getElementById('txn-form');
  var addBtn = document.getElementById('add-line');
  var splitBtn = document.getElementById('split-btn');
  var splitN = document.getElementById('split-n');
  var idx = container.querySelectorAll('.posting').length;

  function rows() {
    return Array.from(container.querySelectorAll('.posting'));
  }
  function q(row, sel) {
    return row.querySelector(sel);
  }
  function parseAmount(s) {
    var n = parseFloat(s);
    return isFinite(n) ? n : 0;
  }

  // The balancing line: the row marked data-balancer="true", else
  // the last row with neither an account nor an amount, else the
  // last row.
  function balancerIndex() {
    var rs = rows();
    for (var i = 0; i < rs.length; i++) {
      if (rs[i].getAttribute('data-balancer') === 'true') return i;
    }
    for (var i = rs.length - 1; i >= 0; i--) {
      var amt = q(rs[i], 'input[name$="[amount]"]');
      var acc = q(rs[i], 'select[name$="[account_id]"]');
      if (amt && !amt.value && acc && !acc.value) return i;
    }
    return rs.length - 1;
  }

  function setBalancerMarker() {
    var rs = rows();
    var bi = balancerIndex();
    rs.forEach(function (r, i) {
      r.classList.toggle('is-balancer', i === bi);
      var badge = r.querySelector('.balancer-badge');
      if (badge) badge.hidden = i !== bi;
    });
    return bi;
  }

  function recompute() {
    var rs = rows();
    var bi = setBalancerMarker();
    var debits = 0;
    var credits = 0;
    var anyAmount = false;
    rs.forEach(function (r, i) {
      var dir = q(r, 'select[name$="[direction]"]');
      var amt = q(r, 'input[name$="[amount]"]');
      if (!dir || !amt) return;
      var v = parseAmount(amt.value);
      if (v !== 0) anyAmount = true;
      if (i === bi) return;
      if (dir.value === 'DEBIT') debits += v;
      else if (dir.value === 'CREDIT') credits += v;
    });
    var net = debits - credits;
    var bal = rs[bi];
    var balDir = bal && q(bal, 'select[name$="[direction]"]');
    var balAmt = bal && q(bal, 'input[name$="[amount]"]');
    if (balDir && balAmt) {
      if (net === 0) {
        balAmt.value = '';
        balDir.value = 'DEBIT';
      } else if (net > 0) {
        balAmt.value = net.toFixed(2);
        balDir.value = 'CREDIT';
      } else {
        balAmt.value = (-net).toFixed(2);
        balDir.value = 'DEBIT';
      }
    }
    // Report the actual state AFTER the balancer is filled: the
    // transaction is balanced whenever the balancer has computed an
    // amount. Only a defensive mismatch shows the shortfall.
    var actualDebits = 0;
    var actualCredits = 0;
    rs.forEach(function (r) {
      var dir = q(r, 'select[name$="[direction]"]');
      var amt = q(r, 'input[name$="[amount]"]');
      if (!dir || !amt) return;
      var v = parseAmount(amt.value);
      if (dir.value === 'DEBIT') actualDebits += v;
      else if (dir.value === 'CREDIT') actualCredits += v;
    });
    updateStatus(actualDebits - actualCredits, anyAmount);
  }

  function updateStatus(net, anyAmount) {
    var small = document.getElementById('split-balance');
    var big = document.getElementById('balance-status');
    if (small) {
      if (!anyAmount) {
        small.textContent = '—';
        small.className = 'text-xs text-slate-500';
      } else if (net === 0) {
        small.textContent = '✓ balanced';
        small.className = 'text-xs text-emerald-600';
      } else {
        small.textContent = 'net ' + net.toFixed(2);
        small.className = 'text-xs text-rose-600';
      }
    }
    if (big) {
      if (!anyAmount) {
        big.textContent = '—';
        big.className = 'text-xs text-slate-500';
      } else if (net === 0) {
        big.textContent = '✓ balanced — ready to save';
        big.className = 'text-xs font-medium text-emerald-600';
      } else {
        var side = net > 0 ? 'credit' : 'debit';
        big.textContent = 'needs ' + Math.abs(net).toFixed(2) + ' on the ' + side + ' side';
        big.className = 'text-xs font-medium text-amber-600';
      }
    }
  }

  function rewriteIndices(row, i) {
    row.querySelectorAll('input, select').forEach(function (el) {
      var name = el.getAttribute('name');
      if (name) {
        el.setAttribute('name', name.replace(/lines\[\d+\]/, 'lines[' + i + ']'));
      }
    });
  }

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
    clone.classList.remove('is-balancer');
    clone.setAttribute('data-balancer', 'false');
    var badge = clone.querySelector('.balancer-badge');
    if (badge) badge.hidden = true;
    container.appendChild(clone);
    idx++;
    return clone;
  }

  // The user typed on the balancer itself: turn it into a fixed line
  // and hand the balancing role to the next empty line, or a new one.
  function promoteBalancer(row) {
    var rs = rows();
    for (var i = 0; i < rs.length; i++) {
      if (rs[i] === row) continue;
      var amt = q(rs[i], 'input[name$="[amount]"]');
      var acc = q(rs[i], 'select[name$="[account_id]"]');
      if (amt && !amt.value && acc && !acc.value) {
        row.setAttribute('data-balancer', 'false');
        rs[i].setAttribute('data-balancer', 'true');
        recompute();
        return;
      }
    }
    var blank = appendBlankRow();
    if (blank) {
      row.setAttribute('data-balancer', 'false');
      blank.setAttribute('data-balancer', 'true');
      recompute();
    }
  }

  container.addEventListener('input', function (ev) {
    var target = ev.target;
    if (!target || !target.name) return;
    var row = target.closest('.posting');
    if (!row) return;
    if (
      (target.name.endsWith('[amount]') || target.name.endsWith('[direction]')) &&
      rows().indexOf(row) === balancerIndex()
    ) {
      row.setAttribute('data-balancer', 'false');
      promoteBalancer(row);
      return;
    }
    recompute();
  });

  container.addEventListener('change', function (ev) {
    if (ev.target && ev.target.name && ev.target.name.endsWith('[account_id]')) {
      recompute();
    }
  });

  if (addBtn) {
    addBtn.addEventListener('click', function () {
      appendBlankRow();
      recompute();
    });
  }

  if (splitBtn) {
    splitBtn.addEventListener('click', function () {
      var n = parseInt(splitN && splitN.value, 10) || 2;
      if (n < 2) n = 2;
      if (n > 50) n = 50;
      var existing = rows().length;
      for (var i = existing; i < n; i++) {
        appendBlankRow();
      }
      // The last row becomes the balancer.
      var rs = rows();
      rs.forEach(function (r) {
        r.setAttribute('data-balancer', 'false');
      });
      rs[rs.length - 1].setAttribute('data-balancer', 'true');
      recompute();
    });
  }

  // Initialise: default the last line to the balancer.
  var rs = rows();
  rs.forEach(function (r) {
    r.setAttribute('data-balancer', 'false');
  });
  if (rs.length) {
    rs[rs.length - 1].setAttribute('data-balancer', 'true');
  }
  recompute();

  // ---- Simple / Advanced mode -------------------------------------
  // (`a12-transaction-entry-ease` transaction-simple-entry)
  var simplePanel = document.getElementById('simple-entry');
  var advancedPanel = document.getElementById('advanced-entry');
  var modeButtons = document.querySelectorAll('[data-entry-mode]');
  var simpleType = document.getElementById('simple-type');
  var simpleAmount = document.getElementById('simple-amount');
  var simpleAccount = document.getElementById('simple-account');
  var simpleCategory = document.getElementById('simple-category');
  var simpleFrom = document.getElementById('simple-from');
  var simpleTo = document.getElementById('simple-to');
  var simpleCategoryRow = document.getElementById('simple-category-row');
  var simpleTransferRow = document.getElementById('simple-transfer-row');
  var simplePayLabelText = document.getElementById('simple-pay-label-text');

  function isSimpleMode() {
    return simplePanel && simplePanel.hidden === false;
  }

  function onSimpleTypeChange() {
    var t = simpleType.value;
    var isTransfer = t === 'transfer';
    if (simpleTransferRow) simpleTransferRow.hidden = !isTransfer;
    if (simpleCategoryRow) simpleCategoryRow.hidden = isTransfer;
    if (simplePayLabelText) {
      simplePayLabelText.textContent = t === 'income' ? 'Received into' : 'Paid from';
    }
    var want = t === 'income' ? 'INCOME' : 'EXPENSE';
    if (simpleCategory) {
      simpleCategory.querySelectorAll('option[data-account-type]').forEach(function (o) {
        o.disabled = o.getAttribute('data-account-type') !== want;
      });
    }
    simpleAmount.required = true;
    if (simpleCategory) simpleCategory.required = !isTransfer;
    if (simpleAccount) simpleAccount.required = !isTransfer;
    if (simpleFrom) simpleFrom.required = isTransfer;
    if (simpleTo) simpleTo.required = isTransfer;
    updateSimpleStatus();
  }

  function toggleRowsDisabled(disabled) {
    rows().forEach(function (r) {
      r.querySelectorAll('input, select').forEach(function (el) {
        el.disabled = disabled;
      });
    });
  }

  function setRow(r, account, dir) {
    var acc = q(r, 'select[name$="[account_id]"]');
    var d = q(r, 'select[name$="[direction]"]');
    var a = q(r, 'input[name$="[amount]"]');
    if (acc) acc.value = account;
    if (d) d.value = dir;
    if (a) a.value = simpleAmount ? simpleAmount.value : '';
  }

  // Simple fields → the two advanced lines (Advanced view).
  function preserveToAdvanced() {
    if (!simpleAmount || !simpleAmount.value) return;
    if (rows().length < 2) return;
    var t = simpleType.value;
    if (t === 'transfer') {
      if (!simpleFrom.value || !simpleTo.value) return;
      setRow(rows()[0], simpleTo.value, 'DEBIT');
      setRow(rows()[1], simpleFrom.value, 'CREDIT');
    } else if (t === 'income') {
      if (!simpleCategory.value || !simpleAccount.value) return;
      setRow(rows()[0], simpleAccount.value, 'DEBIT');
      setRow(rows()[1], simpleCategory.value, 'CREDIT');
    } else {
      if (!simpleCategory.value || !simpleAccount.value) return;
      setRow(rows()[0], simpleCategory.value, 'DEBIT');
      setRow(rows()[1], simpleAccount.value, 'CREDIT');
    }
  }

  // First two advanced lines → simple fields (Simple view).
  function preserveToSimple() {
    if (!simpleAmount || !simpleType) return;
    var rs = rows();
    if (rs.length < 2) return;
    var a0 = q(rs[0], 'select[name$="[account_id]"]');
    var a1 = q(rs[1], 'select[name$="[account_id]"]');
    var amt0 = q(rs[0], 'input[name$="[amount]"]');
    if (!a0 || !a1 || !amt0) return;
    if (!a0.value || !a1.value || !amt0.value) return;
    simpleAmount.value = amt0.value;
    var d0 = q(rs[0], 'select[name$="[direction]"]');
    if (d0 && d0.value === 'DEBIT') {
      simpleType.value = 'expense';
      if (simpleCategory) simpleCategory.value = a0.value;
      if (simpleAccount) simpleAccount.value = a1.value;
    } else {
      simpleType.value = 'income';
      if (simpleAccount) simpleAccount.value = a0.value;
      if (simpleCategory) simpleCategory.value = a1.value;
    }
    onSimpleTypeChange();
  }

  function updateSimpleStatus() {
    var big = document.getElementById('balance-status');
    if (!big) return;
    var amt = simpleAmount && simpleAmount.value;
    if (!amt) {
      big.textContent = '—';
      big.className = 'text-xs text-slate-500';
      return;
    }
    var t = simpleType.value;
    var ok = t === 'transfer'
      ? (simpleFrom.value && simpleTo.value)
      : (simpleCategory.value && simpleAccount.value);
    if (ok) {
      big.textContent = '✓ balanced — ready to save';
      big.className = 'text-xs font-medium text-emerald-600';
    } else {
      big.textContent = 'pick both accounts to finish';
      big.className = 'text-xs font-medium text-amber-600';
    }
  }

  function setMode(mode) {
    if (!simplePanel || !advancedPanel) return;
    var simple = mode === 'simple';
    if (simple) {
      preserveToSimple();
      toggleRowsDisabled(true);
    } else {
      preserveToAdvanced();
      toggleRowsDisabled(false);
      recompute();
    }
    simplePanel.hidden = !simple;
    advancedPanel.hidden = simple;
    modeButtons.forEach(function (b) {
      var active = b.getAttribute('data-entry-mode') === mode;
      b.classList.toggle('is-active', active);
      b.classList.toggle('bg-slate-900', active);
      b.classList.toggle('text-white', active);
      b.classList.toggle('bg-white', !active);
      b.classList.toggle('text-slate-600', !active);
    });
    if (simple) updateSimpleStatus();
  }

  if (simplePanel && advancedPanel) {
    modeButtons.forEach(function (b) {
      b.addEventListener('click', function () {
        setMode(b.getAttribute('data-entry-mode'));
      });
    });
    if (simpleType) {
      simpleType.addEventListener('change', onSimpleTypeChange);
      onSimpleTypeChange();
    }
    if (simpleAmount) {
      simpleAmount.addEventListener('input', updateSimpleStatus);
    }
    [simpleAccount, simpleCategory, simpleFrom, simpleTo].forEach(function (el) {
      if (el) el.addEventListener('change', updateSimpleStatus);
    });
    // Simple is the default view: keep the advanced rows disabled so
    // they don't submit alongside the simple-built lines.
    toggleRowsDisabled(true);
  }

  // Build hidden `lines[N]` inputs from the simple fields; returns
  // the effective lines [[account, direction, amount], …] or null.
  function buildSimpleLines() {
    var t = simpleType.value;
    var amt = simpleAmount.value;
    var cat = simpleCategory.value;
    var pay = simpleAccount.value;
    var from = simpleFrom.value;
    var to = simpleTo.value;
    var lines;
    if (t === 'transfer') {
      if (!from || !to || !amt) return null;
      lines = [[to, 'DEBIT'], [from, 'CREDIT']];
    } else if (t === 'income') {
      if (!cat || !pay || !amt) return null;
      lines = [[pay, 'DEBIT'], [cat, 'CREDIT']];
    } else {
      if (!cat || !pay || !amt) return null;
      lines = [[cat, 'DEBIT'], [pay, 'CREDIT']];
    }
    form.querySelectorAll('input[name^="lines["]').forEach(function (el) {
      if (el.type === 'hidden') el.remove();
    });
    lines.forEach(function (ln, i) {
      [['account_id', ln[0]], ['direction', ln[1]], ['amount', amt]].forEach(function (pair) {
        var inp = document.createElement('input');
        inp.type = 'hidden';
        inp.name = 'lines[' + i + '][' + pair[0] + ']';
        inp.value = pair[1];
        form.appendChild(inp);
      });
    });
    return lines;
  }

  // Submit: build simple lines, guard same-account and far-future
  // dates (`ux-transaction-entry`), then POST via fetch so the
  // multipart body can carry the CSRF header
  // (`a12-transaction-entry-ease` inline documents).
  if (form) {
    var submitAction = 'save';
    form.querySelectorAll('button[name="action"]').forEach(function (b) {
      b.addEventListener('click', function () {
        submitAction = b.value;
      });
    });

    form.addEventListener('submit', function (ev) {
      ev.preventDefault(); // we always submit via fetch
      var effective;
      if (isSimpleMode()) {
        effective = buildSimpleLines();
        if (!effective) {
          window.alert('Please fill in the amount and both accounts.');
          return;
        }
      } else {
        effective = rows().map(function (r) {
          var acc = q(r, 'select[name$="[account_id]"]');
          var dir = q(r, 'select[name$="[direction]"]');
          var amt = q(r, 'input[name$="[amount]"]');
          return [acc ? acc.value : '', dir ? dir.value : '', amt ? amt.value : ''];
        });
      }

      var byAccount = {};
      effective.forEach(function (ln) {
        if (!ln[0]) return;
        if (!byAccount[ln[0]]) byAccount[ln[0]] = { d: false, c: false };
        if (ln[1] === 'DEBIT') byAccount[ln[0]].d = true;
        else if (ln[1] === 'CREDIT') byAccount[ln[0]].c = true;
      });
      var badId = null;
      Object.keys(byAccount).forEach(function (id) {
        if (byAccount[id].d && byAccount[id].c) badId = id;
      });
      if (badId) {
        var opt = container.querySelector('option[value="' + badId + '"]');
        var label = opt ? opt.textContent : 'this account';
        window.alert('This entry moves money within "' + label + '" (the same account on both sides) — pick a different account for one of the lines.');
        return;
      }

      var dateEl = form.querySelector('input[name="date"]');
      if (dateEl && dateEl.value) {
        var d = new Date(dateEl.value + 'T00:00:00');
        var today = new Date();
        today.setHours(0, 0, 0, 0);
        var days = Math.round((d - today) / 86400000);
        if (days > 30) {
          if (!window.confirm('You are recording a transaction dated ' + dateEl.value + ' (' + days + ' days in the future) — is that intentional?')) {
            return;
          }
        }
      }

      var fd = new FormData(form);
      fd.append('action', submitAction);
      var meta = document.querySelector('meta[name="csrf-token"]');
      var headers = {};
      if (meta) headers['X-CSRF-Token'] = meta.getAttribute('content');
      // Use getAttribute('action'): `form.action` is shadowed by the
      // buttons named "action" (named-property access) and would
      // resolve to [object RadioNodeList].
      fetch(form.getAttribute('action'), { method: 'POST', headers: headers, body: fd })
        .then(function (r) { return r.text(); })
        .then(function (html) {
          // The server replies with the transaction show page
          // (success) or the re-rendered form (error); replace the
          // document so the user sees either.
          document.open();
          document.write(html);
          document.close();
          // Fix the address bar to the show page when saved.
          var m = html.match(/\/ledgers\/([0-9a-f-]+)\/transactions\/([0-9a-f-]+)\/documents/);
          if (m && html.indexOf('id="txn-form"') === -1) {
            history.replaceState(null, '', '/ledgers/' + m[1] + '/transactions/' + m[2]);
          }
        })
        .catch(function () {
          window.alert('Save failed — please try again.');
        });
    });
  }
})();
