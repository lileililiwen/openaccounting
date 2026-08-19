# Novice-User Form Test — 2026-08-19

Methodology and evidence for a novice-user UX pass over the form submission flow.

## Environment

- App: OpenAccounting running locally at `http://127.0.0.1:3002` (`APP_PORT=3002` in `.env`).
  The task URL `http://localhost:5173/form` was **not** reachable: nothing listens on 5173 (3000 is taken by an unrelated service). `/form` itself is not a route → 404 (`00_form_url.png`).
- Browser: Google Chrome (headless), viewport 1280×900, **system language zh-CN**.
- Automation: Playwright 1.62 (via `node_modules/omniroute/node_modules/playwright`, executable `/usr/bin/google-chrome-stable`).

## Persona

First-time user ("newbie"):
- does **not** read helper text,
- types arbitrary/random data,
- closes any dialog immediately without reading it (all `confirm()` dialogs auto-accepted),
- picks the first option in dropdowns,
- submits early to "see what happens".

## Procedure

1. Visit `/form` → expect a route; observe 404 behaviour.
2. Land on `/` → redirected to `/login`.
3. Attempt a random login (`random.person@example.com` / `123456`) → observe error handling.
4. Register a new user:
   - first with a short password (5 chars) → observe validation feedback;
   - then with a valid 12+ char password → observe post-register redirect.
5. Sign in again, create a ledger ("my books").
6. Open the transactions list, then the New Transaction form.
7. Submit with an empty form, then with a single 5.50 posting line, then with the same account on both sides → observe validation, balance indicator, and the saved result.

## Screenshots

| File | Step | Key observation |
|---|---|---|
| `00_form_url.png` | `/form` | Chrome default 404 (Chinese), no branded page |
| `01_root.png` | `/` → `/login` | English login page |
| `02_random_login_error.png` | bad login | "Invalid email or password"; both fields cleared |
| `03_register_page.png` | register form | English form |
| `04_register_short_pw.png` | short password | browser tooltip in Chinese; no inline error |
| `05_after_register.png` | after register | redirected to `/login?next=/ledgers/new` (not auto-logged-in) |
| `10_after_login.png` | after sign-in | lands on `/ledgers` (next target lost) |
| `11_ledger_new.png` | new ledger | form, accrual/cash explanation |
| `12_after_ledger_create.png` | ledger show | sidebar, "Enable append-only" with `confirm()` |
| `20_txn_list.png` | transactions list | empty state uses "debit and credit" |
| `21_txn_new_form.png` | new transaction | "✓ balanced" on empty form; 19-account flat dropdown; Debit/Credit jargon; equal-weight Save/Save-as-draft |
| `22_save_empty.png` | empty submit | Chinese "请填写此字段" tooltip |
| `23_save_unbalanced.png` | 1-line submit | "net 5.50"; Chinese "请在列表中选择一项" |
| `24_save_both_debit.png` | both legs same account | saved `Cash on Hand → Cash on Hand` (meaningless); Chinese "选择文件" picker |

## Findings → Specs

| # | Finding | Spec | Evidence |
|---|---|---|---|
| 1 | Browser validation tooltips in Chinese vs English UI | `ux-language-consistency` | `04`, `22`, `23` |
| 2 | Native file picker in Chinese | `ux-language-consistency` | `24` |
| 3 | No branded 404 | `ux-language-consistency` | `00` |
| 4 | "✓ balanced" shown on empty form | `ux-transaction-entry` | `21` |
| 5 | Same-account both-leg transaction saved silently | `ux-transaction-entry` | `24` |
| 6 | Far-future date accepted silently | `ux-transaction-entry` | `24` |
| 7 | Flat 19-account dropdown | `ux-transaction-entry` | `21` |
| 8 | Debit/Credit jargon without help | `ux-transaction-entry` | `21` |
| 9 | Save vs Save-as-draft equal weight | `ux-transaction-entry` | `21` |
| 10 | No auto-login after register; next target lost | `ux-onboarding-flow` | `05`, `10` |
| 11 | Short-password feedback only via browser tooltip | `ux-onboarding-flow` | `04` |
| 12 | Login error clears email | `ux-onboarding-flow` | `02` |
| 13 | Empty state uses "debit and credit" | `ux-onboarding-flow` | `20` |
| 14 | Append-only enable is a single `confirm()` | `ux-onboarding-flow` | `12` |

## Reproducing

Playwright probe scripts used for this run are kept at `/tmp/oa-shots/probe{1,2,3}.js` (not part of the repo).
They depend on the running server, a fresh user, and the seeded default chart of accounts.
