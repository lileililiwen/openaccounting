# menu-navigation Specification

## Purpose
TBD - created by archiving change 2026-08-19-u11-menu-navigation. Update Purpose after archive.
## Requirements
### Requirement: Active-State Indicator

The top navigation bar SHALL visualise the current top-level
section with a persistent, animated indicator that slides between
items whenever the section changes. The indicator MUST be
rendered as a single transformable element, not a per-link
background, and its position SHALL update on the active item
after full-page navigation.

Visual requirements:

- The active item SHALL be visibly distinct from hover state by
  at least one non-color cue (e.g. an underline, a left border
  bar, or a persistent background).
- The indicator SHALL animate between items with a CSS
  `transition` on `transform` (or `left`/`width`) of 100–200 ms
  `ease-out`.
- The indicator SHALL be visible in both light and dark modes.

#### Scenario: Active state on a top-level section

- **WHEN** the user navigates to `/ledgers/{id}/transactions`
- **THEN** the top-nav link whose target is the Transactions
  section SHALL have the `is-active` class and the sliding
  indicator SHALL be positioned over that link.

#### Scenario: Active state survives a page reload

- **WHEN** the user reloads `/ledgers/{id}/reports`
- **THEN** the Reports link SHALL be the active item on first
  paint, with no flash of the wrong active state.

### Requirement: Section Grouping

Within a ledger, the top navigation SHALL group the per-ledger
links into four clusters, presented in this order: **Record**,
**Plan**, **Analyze**, **Admin**. Each cluster SHALL have a small
uppercase label and SHALL contain the following items:

- **Record**: Transactions, Documents, Bank Feeds, Import,
  WeChat, Alipay.
- **Plan**: Accounts, Approvals, Expenses.
- **Analyze**: Reports, Dashboard.
- **Admin**: the Admin link (only when the user has role
  `admin`).

The grouping MAY be implemented as a flat list with section
headers, or as a sidebar; the structure required here is the
information architecture, not the layout.

#### Scenario: All four clusters present inside a ledger

- **WHEN** the user is on `/ledgers/{id}/dashboard` and has role
  `admin`
- **THEN** the navigation SHALL contain the four cluster labels
  in the order above, and the Admin cluster SHALL contain the
  Admin link.

#### Scenario: Admin cluster hides for non-admins

- **WHEN** the user has role `editor`
- **THEN** the Admin cluster SHALL NOT be rendered.

### Requirement: Icons on Nav Items

Every top-nav link SHALL be paired with a small icon (16–20 px)
to the left of the label. The icon SHALL inherit the link's
`color` so it automatically follows the active, hover, and
disabled states.

#### Scenario: Icon present on every link

- **WHEN** the user renders any page that includes the top nav
- **THEN** every link inside the four clusters SHALL have an
  adjacent `<img>` or inline `<svg>` element with non-empty
  dimensions and an `aria-hidden="true"` attribute.

### Requirement: Breadcrumb

A breadcrumb component SHALL be rendered immediately below the
top navigation on every authenticated page. The chain SHALL be:

- `Ledgers` — links to `/ledgers`.
- `{Ledger Name}` — links to `/ledgers/{id}/dashboard` (only
  when inside a ledger).
- `{Section}` — the current top-level section, rendered as the
  current item (not a link).

The breadcrumb SHALL be keyboard-navigable: each link SHALL be
reachable via Tab and SHALL have a visible focus ring.

#### Scenario: Breadcrumb on a top-level section page

- **WHEN** the user is on `/ledgers/{id}/transactions`
- **THEN** the breadcrumb SHALL show three items:
  `Ledgers › {Ledger Name} › Transactions`, with the third item
  rendered as a non-link `<span aria-current="page">`.

#### Scenario: Breadcrumb on a non-ledger page

- **WHEN** the user is on `/account`
- **THEN** the breadcrumb SHALL show a single item
  `Account`, rendered as the current page.

### Requirement: Skeleton Transition on Navigation

When the user clicks an in-app link, the content area SHALL
display a skeleton placeholder for 150–200 ms before the new
page renders. The skeleton SHALL be visually distinct from the
loaded content (e.g. a 3–4 row block of low-opacity rectangles)
and SHALL fade out as the real content fades in.

The skeleton MUST NOT block user interaction with the top nav
or the breadcrumb; clicking another nav item during the
skeleton phase SHALL cancel the pending navigation and start a
new one.

#### Scenario: Click triggers a skeleton

- **WHEN** the user clicks the Reports link while on
  `/ledgers/{id}/transactions`
- **THEN** the content area SHALL show a skeleton placeholder
  for the duration of the navigation.

### Requirement: Persistent Sidebar Inside a Ledger

Once the user is inside a ledger, the four clusters SHALL also
be rendered in a persistent left sidebar. The sidebar SHALL:

- remain visible on `md+` viewports;
- be collapsible to an icon-only rail via a toggle button;
- have a sticky position with `overflow-y: auto`, so the
  sidebar scroll is independent of the document scroll.

The sidebar's content SHALL be the same set of links as the
top-nav clusters, not a duplicate set of deep links.

#### Scenario: Sidebar appears inside a ledger

- **WHEN** the user navigates to `/ledgers/{id}/dashboard` on a
  ≥768 px viewport
- **THEN** the page SHALL render a left sidebar with the four
  cluster sections and the items listed in Requirement 2.

#### Scenario: Sidebar collapses to icons

- **WHEN** the user clicks the collapse toggle
- **THEN** the sidebar width SHALL shrink to the icon-only width
  and the labels SHALL be hidden but the icons SHALL remain
  visible.

#### Scenario: Sidebar scroll is independent

- **WHEN** the user scrolls the page content while the sidebar
  contains more items than fit
- **THEN** the sidebar SHALL scroll independently of the page
  content; the sidebar's scroll position SHALL NOT be affected
  by the page scroll position.

### Requirement: Dark-Mode Parity

Every new style introduced for the menu (active-state indicator,
cluster labels, sidebar background, breadcrumb separators) SHALL
have a `dark:` variant that renders the same affordance in dark
mode. The visual hierarchy SHALL be the same in light and dark
modes: the active item is the most prominent, hover is
secondary, and inactive items are the least prominent.

#### Scenario: Active state in dark mode

- **WHEN** the user's theme is set to dark and the user is on
  `/ledgers/{id}/transactions`
- **THEN** the active-state indicator SHALL be visible with a
  contrast ratio of at least 3:1 against the top-bar background.

### Requirement: Stable Navigation Context

Crossing into a child view (e.g. opening a single transaction
from the transactions list, opening a single account from the
accounts list) MUST NOT change which top-nav item is styled as
active. The top-nav active state is keyed off the top-level
section, not the URL path. This invariant is the anti-regression
test for the Frappe v16 sidebar auto-switching complaint
(`frappe/frappe#36317`).

#### Scenario: Child view keeps the parent's active state

- **WHEN** the user navigates from `/ledgers/{id}/transactions`
  to `/ledgers/{id}/transactions/{txn_id}`
- **THEN** the top-nav Transactions link SHALL remain the
  active item; no other top-nav link SHALL gain the `is-active`
  class.

#### Scenario: Linked entity does not re-parent the active item

- **WHEN** the user is on `/ledgers/{id}/transactions` and
  clicks a link to an Account that lives under another
  top-level section
- **THEN** the Transactions link SHALL remain the active item
  until the user explicitly navigates to a different top-level
  section.

