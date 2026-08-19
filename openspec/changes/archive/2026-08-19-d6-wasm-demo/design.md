# ## Context

A demo is the cheapest acquisition channel.

## Goals / Non-Goals

**Goals:**
- One-click try.

**Non-Goals:**
- Full server parity (write to server from the demo is out of scope).

## Decisions

- Reuse the `domain` crate.
- WASM-bindgen for the JS bridge.
- Tailwind via CDN in the demo only (the production server already uses
  Play CDN in dev).
