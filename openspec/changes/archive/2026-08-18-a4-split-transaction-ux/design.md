# ## Context

The existing form already accepts any number of rows. The UX is the gap.

## Goals / Non-Goals

**Goals:**
- One-click split expansion.
- Live imbalance display.
- Works with JS disabled (server still validates).

**Non-Goals:**
- Full WYSIWYG.
- Multi-currency splits (separate change).

## Decisions

- 50 lines of vanilla JS — no build step.
- Live display is a CSS counter; no React-style virtual DOM.
