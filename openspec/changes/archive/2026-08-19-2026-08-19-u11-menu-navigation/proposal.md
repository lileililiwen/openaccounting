# Menu Navigation UX

## Why

`templates/partials/_nav.html` is a flat sticky top bar with up to
11 unstyled links per ledger (Dashboard, Transactions, Accounts,
Documents, Reports, Import, Expenses, Approvals, WeChat, Alipay,
Bank Feeds). It has no active-state styling, no iconography, no
grouping, no breadcrumb, no animation, and no persistent context
after navigating to a child view. This is the source of the
"disjointed transitions" feedback the user has reported.

A research workflow (102 agents, 25 claims verified against
primary sources) compared openaccounting's menu to the well-regarded
open-source accounting apps:

- **Firefly III** — left sidebar with grouped icon+label modules and
  persistent parent context. The maintainer has explicitly declined
  to make the sidebar persist in its collapsed state across
  navigation, which the community has flagged as a recurring
  friction point (`firefly-iii/firefly-iii#8494`).
- **Akaunting** — left sidebar with grouped Banking / Sales /
  Purchases / Reports / Settings, "icon-and-label modules grouped
  by domain" (canonical docs at
  `akaunting.com/hc/docs/the-user-interface/the-sidebar/`).
- **ERPNext / Frappe Books** — top bar + Workspace sidebar; the
  v16 redesign removed v15's stable breadcrumbs and now
  *auto-switches* the sidebar to the module of any linked DocType
  the user clicks. The community has labelled this behaviour
  "disorienting" and "we are spending time searching for things
  and getting lost" (verbatim comment by Bradeskojest on
  `frappe/frappe#36317`, corroborated by four other users and
  acknowledged by Frappe team member `@sokumon`).
- **Manager.io** — vertical left tab list of ~38 modules + Settings.
  The lead developer (Lubos) and forum user Ebrahim defend the flat
  structure on the grounds that "abandoning the easy flat structure
  … would make searching a bit more difficult for new users"
  (`forum.manager.io/t/menu-categorization/36874`). The flat
  approach has its own failure modes: the tab list scrolls with
  the page content and "is inconvenient" (`forum.manager.io/t/manager-navigation-feedback/34621`,
  post #3 by Patch).
- **Odoo Accounting** — left sidebar with collapsible apps and a
  module switcher. Users complain about overwhelming menus and slow
  load times for the Accounting sub-menus
  (`reddit.com/r/Odoo/comments/1deb36y`,
  `odoo.com/forum/help-1/very-slow-loading-when-opening-accounting-menus-despite-sufficient-server-resources-od`).
- **GnuCash, KMyMoney, Skrooge** — desktop menu-bar + Account Tree
  pane. A stable mature pattern that influenced the desktop
  reference documentation for the surveyed apps
  (`gnucash.org/docs/v5/C/gnucash-manual/GUIMenus.html`, `gui-windows.html`).
- **ihatemoney** — deliberately minimal single-page app. The
  polar opposite of the ERP pattern; useful only as a counterexample
  on why an accounting app needs structure.

The workloads of the apps with the strongest ratings converge on a
common set of decisions:

1. A persistent active-state indicator that visually anchors the
   current section (Frappe's left border bar, Akaunting's
   highlighted parent, NN/G's indicator guidance).
2. Modules grouped by domain rather than alphabetised flat
   (Akaunting's Banking / Sales / Purchases / Reports / Settings;
   Manager.io's repeated user request for the same).
3. Icon + label pairing on every nav item (every well-regarded app).
4. A breadcrumb below the top nav to restore location context
   (NN/G: "Breadcrumbs: 11 Design Guidelines for Desktop and
   Mobile", `nngroup.com/articles/breadcrumbs/`).
5. An animated indicator / transition on navigation (recurring
   theme in the Frappe `Redesigning Desk` thread).
6. A persistent sidebar once inside a ledger, collapsible to
   icons-only (Firefly III, Akaunting, Frappe consensus).

The most-cited failure mode to avoid is **auto-switching navigation
context on cross-module clicks** — opening a linked Customer from
the Accounting workspace should not silently re-root the sidebar
to the CRM workspace, because users "lose track of where they
started" (`frappe/frappe#36317`, comment by tamburro92).

## What Changes

- Add a persistent, sliding active-state indicator to the top nav
  (one pseudo-element that translates between items, animated
  with CSS transitions).
- Group the 11 per-ledger links into four domain clusters in the
  template: **Record** (Transactions, Documents, Bank Feeds, Import,
  WeChat, Alipay), **Plan** (Accounts, Approvals, Expenses),
  **Analyze** (Reports, Dashboard), **Admin** (Admin link, when
  role is admin).
- Pair every nav link with an icon (Heroicons / Lucide, vendored
  SVG inline — no new dependency).
- Add a breadcrumb component below the top nav showing the chain
  `Ledgers › {Ledger Name} › {Section}` (or `Account` for the
  non-ledger pages).
- Add a 150–200 ms skeleton-and-fade transition on in-app
  navigation so the click feels acknowledged before the page is
  rendered.
- Once the user is inside a ledger, render the four clusters in
  a persistent left sidebar that is collapsible to an icon-only
  rail. The top bar keeps the wordmark, ledger switcher, and
  user menu.
- Ensure every new style has a `dark:` parity class so dark mode
  does not regress (the existing top nav already has dark-mode
  styles; the new active-state styling must too).
- Add an anti-regression invariant: crossing into a child view
  (e.g. opening a single transaction from the transactions list)
  MUST NOT change which top-nav item is styled as active. The
  top-nav active state is keyed off the top-level section, not
  the URL path.

## Capabilities

### New Capabilities

- `menu-navigation`: top-nav active state, section grouping,
  icons, breadcrumb, skeleton transition, persistent sidebar,
  dark-mode parity, and stable navigation context.

## Impact

**New files:**
- `openspec/changes/2026-08-19-u11-menu-navigation/specs/menu-navigation/spec.md`
  (this change's normative spec).
- `templates/partials/_breadcrumb.html` — new partial.
- `templates/partials/_sidebar.html` — new per-ledger sidebar
  partial.
- `static/icons/nav/*.svg` — vendored icon set (one per nav item).
- `static/css/nav.css` — active-state indicator animation,
  sidebar collapse transition, dark-mode parity overrides. (The
  project uses Tailwind Play CDN for utility classes; CSS for
  transitions lives in the vendored `static/css/app.css` or a
  sibling file.)
- `tests/integration/menu_navigation.rs` — HTTP integration tests
  covering the active-state invariant (Requirement 8) and the
  new components.

**Modified files:**
- `templates/partials/_nav.html` — restructure into the four
  clusters, add icons, add the active-state container, and embed
  the breadcrumb partial.
- `templates/ledgers/show.html` and other base templates that
  include `_nav.html` (no semantic change; the partial handles
  the difference between per-ledger and global mode).
- `static/css/app.css` (or `templates/base.html` if Tailwind
  handles the new utilities) — add the active-state pseudo-element
  and the sidebar transition classes.
- `openspec/specs/web-ui/spec.md` — the existing
  `### Requirement: Navigation` is shortened to a delegation;
  the new `menu-navigation` spec is the authoritative source.

## Non-Goals

- A command palette / kbar-style quick switcher. Future change.
- Drag-and-drop reordering of nav items. Future change,
  out of scope here.
- Per-user configurable visibility of nav items. Future change.
- Native mobile shell rework. The existing
  `2026-08-14-2026-08-14-mobile-shells` change handles native
  packaging; the new sidebar respects the mobile sub-bar pattern
  laid out in `web-ui/spec.md`.
- Adopting a mega-menu or grouped megamenu pattern. The
  `u5-dashboard-widgets`-style "group by domain" labelling is
  the consensus choice; ERP-style mega menus are over-engineered
  for 11 items.
