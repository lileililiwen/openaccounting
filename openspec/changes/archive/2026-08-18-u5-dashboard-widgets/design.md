# ## Context

Fixed layout.

## Goals / Non-Goals

**Goals:**
- Personalization.

**Non-Goals:**
- Custom widget authoring.

## Decisions

- Widgets are server-rendered; the picker is a small form.
- Each widget is a function `fn widget(ctx: &Ledger) -> Html`.

