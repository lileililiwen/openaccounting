# Menu Navigation UX — Design

## Context

`templates/partials/_nav.html` is a flat sticky top bar with up to
11 unstyled links per ledger. It has no active state, no icons, no
grouping, no breadcrumb, and no animation. The cross-section
transition feels disjointed because the user has no visual anchor
that says "you are here" beyond the URL bar.

A research workflow (102 agents, 25 claims verified against
primary sources) surveyed Firefly III, Akaunting, Frappe Books,
ERPNext, Invoice Ninja, Odoo Accounting, Manager.io, ihatemoney,
GnuCash, KMyMoney, and Skrooge, and synthesised the recommendations
mapped below. The full research transcript is preserved under
`.qoder/sessions/.../workflows/runs/wf_248716aa-fa9/`.

## Goals / Non-Goals

**Goals:**
- Persistent top-nav active-state indicator with a slide
  transition.
- Grouping of the 11 per-ledger links into four clusters:
  Record, Plan, Analyze, Admin.
- Icon + label pairing on every nav item.
- Breadcrumb below the top nav.
- Skeleton-and-fade transition on in-app navigation.
- Persistent sidebar inside a ledger, collapsible to icons-only.
- Dark-mode parity on every new style.
- Stable navigation context: child views do not re-parent the
  active top-nav item.

**Non-Goals:**
- Command palette / quick switcher.
- Drag-and-drop reordering of nav items.
- Per-user configurable visibility of nav items.
- Native mobile shell rework (handled by
  `2026-08-14-2026-08-14-mobile-shells`).
- Mega-menu or mega-menu-style grouping.

## Decisions

### Tier 1 — fix the "disjointed" feeling first

1. **Animated active-state indicator.** A single CSS
   pseudo-element (e.g. `::after` on the nav container) whose
   `transform: translateX()` is updated on link hover/active. A
   100–200 ms `transition` on the transform gives the slide. This
   is the single highest-leverage CSS change. Frappe's "left
   border bar on the active module" and Akaunting's persistent
   parent highlight are the cited patterns (`discuss.frappe.io/t/desk-2-0-new-navigation/29888`,
   `uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2`).
2. **Breadcrumb below the top nav.** A small partial that renders
   `Ledgers › {Ledger Name} › {Section}` based on the route. The
   canonical reference is NN/G's breadcrumb guideline
   (`nngroup.com/articles/breadcrumbs/`). This kills the
   "where am I now?" problem after a click.
3. **Skeleton + fade on click.** A 150–200 ms skeleton
   placeholder in the content area on click, then fade in. Avoids
   the perceptible flash when the page is faster than the user's
   mental model.

### Tier 2 — visual polish

4. **Group the 11 links into 4 clusters.** The flat-vs-grouped
   debate on `forum.manager.io/t/menu-categorization/36874`
   resolved on "let users choose" — but with only ~11 items the
   right call is to group by domain. Domain names: **Record**
   (Transactions, Documents, Bank Feeds, Import, WeChat, Alipay),
   **Plan** (Accounts, Approvals, Expenses), **Analyze** (Reports,
   Dashboard), **Admin** (Admin link, admin role only).
5. **Add icons.** Every well-regarded reference (Firefly III,
   Akaunting, Frappe, Invoice Ninja) pairs icons with labels.
   Pairing helps recognition and is the median recommendation in
   `uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2`.
   Vendored inline SVG (Heroicons / Lucide) — no JS dependency.
6. **Dark-mode parity.** The top nav already flips `slate-200`
   borders in dark mode, but the active state is invisible. Add
   `dark:bg-slate-800 dark:text-white` variants for the new
   active-state styling.

### Tier 3 — fluid transitions

7. **Persistent sidebar once inside a ledger.** Move the 11
   per-ledger links into a left sidebar (collapsible to icons-only)
   once the user is inside a ledger. Top bar keeps brand + user.
   This is the Firefly III / Akaunting / Frappe consensus pattern.
   The most-requested Firefly III feature (sidebar persistence
   across navigation, `firefly-iii/firefly-iii#8494`) is the
   single biggest UX improvement any of the surveyed apps have
   been pushed to make.
8. **Stable navigation context.** Anti-regression from the Frappe
   v16 sidebar auto-switching bug
   (`frappe/frappe#36317`). The active top-nav item is keyed off
   the user-visible section, not the URL. Opening a child view
   (single transaction, single account) keeps the parent's
   indicator anchored.

### Implementation choices

- **Template restructuring** — the existing single
  `templates/partials/_nav.html` is replaced by three partials:
  `_nav.html` (top bar), `_breadcrumb.html` (below the bar),
  `_sidebar.html` (left rail). The current mobile sub-bar block
  is reduced to a single "Sections" trigger that opens the
  sidebar as a slide-over drawer on `<md` viewports.
- **Active-state container** — wrap the nav links in a
  `<div class="nav-shell">` with one absolutely-positioned
  `::after` indicator. The container's bounding box is the union
  of the link bounding rects; the indicator's `translateX` and
  `width` are updated by a small JS helper on hover/active. CSS
  only for the animation.
- **Icons** — vendored SVG with `currentColor` so they inherit
  the link color and respect dark mode without extra classes.
- **Tests** — `tests/integration/menu_navigation.rs` covers:
  - the active-state CSS class is rendered on the right nav item
    for each top-level section route;
  - the active-state class does NOT change when navigating to a
    child view (the anti-regression test);
  - the breadcrumb emits the expected chain for representative
    routes;
  - the sidebar emits the four clusters in the expected order.

## Risks / Trade-offs

### Behavioral

- **Auto-switching navigation context.** The single most-complained
  about failure in Frappe v16: opening a linked DocType from
  another module auto-switches the sidebar
  (`frappe/frappe#36317`). User comment by Bradeskojest:
  "We are spending time searching for things and getting lost."
  Mitigated by Requirement 8 — the active top-nav item is keyed
  off the visible section, not the URL. The implementation must
  not pattern-match on URL prefix naively; it must use the
  template's section context.
- **Sidebar scroll tying to page scroll.** Manager.io users
  complain that the left tab list scrolls with the page
  (`forum.manager.io/t/manager-navigation-feedback/34621`,
  post #3 by Patch: "The tabs on the left hand side scrolling is
  inconvenient"). Mitigated by making the sidebar
  `position: sticky` with `overflow-y: auto` and the page
  `overflow-y: visible`. The sidebar's scroll is independent of
  the document's scroll.
- **Over-animation.** Animating every property with
  `transition-all` causes jank and battery drain. Mitigated by
  transitioning only `transform` and `background-color` on the
  indicator, with `will-change: transform` set on the
  pseudo-element.

### Aesthetic

- **Three-clicks-and-Miller's-7±2 are myths.** Manager.io forum
  users cite Miller's 7±2 as grounds for the flat menu. UX
  researchers explicitly debunk this
  (`nngroup.com/articles/3-click-rule/`,
  `uxmyths.com/post/931925744/myth-23-choices-should-always-be-limited-to-seven`).
  Group by domain; do not artificially cap items.
- **Don't over-feature Settings.** Akaunting and Odoo both bury
  Settings under a single low-emphasis link. openaccounting
  currently has no Settings entry at all — adding one goes in
  this change, but it lives below the four functional clusters.
- **Single-user feature requests ≠ consensus.** A research
  refuted finding was that
  `firefly-iii/firefly-iii#10647` represented "users have called
  for" responsive layouts — in fact the issue was filed by a
  single non-contributor with zero reactions and closed as
  duplicate within hours. Use volume + maintainer engagement as
  the signal, not raw issue count.

### Operational

- **CSS bundle size.** Vendored SVGs add to the payload. The
  default acceptable bundle for this project's static assets is
  sized by the existing `static/css/app.css` precedent; the new
  icons are inlined only where used (heroicons-style), not
  included on every page.
- **Browser support.** `transition` on `transform` and
  `position: sticky` are universally supported. No fallback
  required.

## Alternatives Considered

- **Flat menu with search** (Manager.io's defence by Ebrahim in
  `forum.manager.io/t/menu-categorization/36874` post #2). The
  argument is that "abandoning the easy flat structure and
  navigating menus would make searching a bit more difficult for
  new users". Rejected: openaccounting's 11 items do not warrant
  a search-driven interface, and the verified community pain
  points (sidebar scrolls with page, "spending time searching for
  things") are direct consequences of the flat approach.
- **Mega-menu / ERP-style grouping** (proposed in
  `forum.manager.io/t/menu-categorization/36874` post #9 by
  dalacor). Rejected: over-engineered for 11 items; adds a second
  mouse-motion target; the Frappe "Workspaces" pattern is
  equivalent but lighter-weight.
- **Auto-switching sidebar on DocType navigation** (Frappe v16's
  chosen behaviour). Explicitly rejected as the single
  most-cited failure mode in the research.
- **Hamburger menu collapsing the top nav on desktop.** The
  existing `web-ui/spec.md` "Navigation" Requirement already
  mandates a sticky top nav with per-ledger links visible on
  `md+`. A hamburger would violate that and remove the
  active-state indicator the resize contract.

## References

- NN/G breadcrumbs guidelines —
  `nngroup.com/articles/breadcrumbs/`
- NN/G 3-click rule false —
  `nngroup.com/articles/3-click-rule/`
- UX Myths #23 —
  `uxmyths.com/post/931925744/myth-23-choices-should-always-be-limited-to-seven`
- UX Planet sidebar —
  `uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2`
- Frappe Desk 2.0 discussion —
  `discuss.frappe.io/t/desk-2-0-new-navigation/29888`
- Frappe v16 sidebar auto-switching —
  `github.com/frappe/frappe/issues/36317`
- Frappe workspace switcher feedback —
  `github.com/frappe/frappe/issues/35221`
- Firefly III sidebar glitch —
  `github.com/firefly-iii/firefly-iii/issues/8494`
- Manager.io navigation feedback —
  `forum.manager.io/t/manager-navigation-feedback/34621`
- Manager.io menu categorization —
  `forum.manager.io/t/menu-categorization/36874`
- Akaunting sidebar docs —
  `akaunting.com/hc/docs/the-user-interface/the-sidebar/`
- GnuCash menu manual —
  `gnucash.org/docs/v5/C/gnucash-manual/GUIMenus.html`
- ERPNext / Frappe workspace customization —
  `fossibleworks.com/blog/posts/workspace-customization-in-erpnext`
- Research workflow transcript —
  `.qoder/sessions/.../workflows/runs/wf_248716aa-fa9/`
- Full research report — file
  `.qoder/sessions/.../workflows/runs/wf_248716aa-fa9/output.json`
  and the journal `journal.jsonl` for verified claims and
  sources.
