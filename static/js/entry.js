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
})();
