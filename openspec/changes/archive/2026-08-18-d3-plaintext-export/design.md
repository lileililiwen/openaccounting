# ## Context

Builds on O1.

## Goals / Non-Goals

**Goals:**
- Real interoperability with PTA tools.

**Non-Goals:**
- Round-trip preservation of OCR-derived fields.

## Decisions

- CLI subcommand via `clap`.
- Parser: a small hand-written Beancount parser (no FFI to Python).
