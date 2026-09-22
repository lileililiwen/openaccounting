// a11y.js — HTMX focus management + aria-live announcements
// (`u13-ux-a11y-mobile`, finding A1).
//
// After every HTMX partial swap, screen-reader users must learn
// two things: where focus landed, and what changed. Without this,
// an hx-get/hx-post that updates a list leaves the user on a
// stale focus target and silent.
//
// Wire contract between server and client:
//
//   <div data-htmx-focus="#heading-id" data-htmx-announce="Saved 3 transactions.">
//     <h2 id="heading-id">Transactions</h2>
//     … new content …
//   </div>
//
// `data-htmx-focus` is a CSS selector; the helper moves focus
// there. `data-htmx-announce` is the text to read aloud via the
// global aria-live region. The region is created lazily on first
// use so pages without announcements never ship a no-op node.
//
// The helper also covers the keyboard-only post flow used by the
// WCAG audit: when a keyboard user submits the transaction
// editor, the server responds with the show page carrying both
// markers; this script focuses the show-page heading and
// announces the total.

(function () {
  "use strict";

  function ensureLiveRegion() {
    var existing = document.getElementById("oa-live-region");
    if (existing) return existing;
    var region = document.createElement("div");
    region.id = "oa-live-region";
    region.setAttribute("role", "status");
    region.setAttribute("aria-live", "polite");
    region.setAttribute("aria-atomic", "true");
    // Tailwind's sr-only — keep it visually hidden but
    // readable by AT.
    region.className = "sr-only";
    document.body.appendChild(region);
    return region;
  }

  function announce(text) {
    if (!text) return;
    var region = ensureLiveRegion();
    // Clear then re-set so identical consecutive messages
    // still re-fire (AT engines can swallow repeats).
    region.textContent = "";
    // Defer to next tick so the cleared/reset pair is read
    // as two separate events.
    setTimeout(function () {
      region.textContent = text;
    }, 30);
  }

  function focusTarget(selector) {
    if (!selector) return;
    var el = document.querySelector(selector);
    if (!el) return;
    if (!el.hasAttribute("tabindex")) {
      el.setAttribute("tabindex", "-1");
    }
    try {
      el.focus({ preventScroll: false });
    } catch (_) {
      el.focus();
    }
  }

  function processSwap(target) {
    if (!target) return;
    var root = target;
    var focusSel = root.getAttribute("data-htmx-focus");
    var announceText = root.getAttribute("data-htmx-announce");
    if (announceText) announce(announceText);
    if (focusSel) focusTarget(focusSel);
  }

  // HTMX events fire on document when no swap target is
  // specified, or on the swap element when one is. Handle both.
  document.addEventListener("htmx:afterSwap", function (evt) {
    var target = evt.detail && evt.detail.target;
    if (target) {
      processSwap(target);
      return;
    }
    processSwap(evt.target);
  });

  // Fallback for hx-swap="none" / OOB swaps where the response
  // is a single fragment: find markers anywhere in the
  // updated subtree.
  document.addEventListener("htmx:oob-afterSwap", function (evt) {
    if (evt.target) processSwap(evt.target);
  });

  // Expose the helpers so server-rendered pages can trigger
  // the same focus + announce flow on plain loads (e.g. after
  // a 303 redirect to a confirmation page that carries the
  // markers but never went through HTMX).
  window.oaA11y = {
    focus: focusTarget,
    announce: announce,
    process: processSwap,
  };
})();