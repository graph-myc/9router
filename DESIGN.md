# DESIGN.md — Mycelix dashboard design system

> The visual language for the Mycelix dashboard (UI wordmark **M Y C**). Follow this for all
> dashboard work. Tokens live in [`crates/frontend/index.html`](./crates/frontend/index.html) (`<style>`);
> components and the icon system live in [`crates/frontend/src/main.rs`](./crates/frontend/src/main.rs).

## Design language

- **Dark-first, developer-tool aesthetic** — calm near-black surfaces, one warm accent, high legibility.
- **Single accent**: warm orange. Used sparingly for the active state, primary buttons, and the logo.
- **Semantic color only where it carries meaning**: routing strategies and status (never decorative).
- **Monospace for machine values** (URLs, model IDs, code, logs); sans-serif for everything else.
- **Generous spacing**, soft rounded corners, a single centered content column (max 940px).
- **Inline SVG icons, never emoji.** Icons inherit `currentColor`.

## Color tokens

Defined as CSS variables on `:root`:

| Token | Value | Use |
|-------|-------|-----|
| `--bg` | `#0b0b0c` | App background |
| `--panel` | `#161617` | Card / surface background |
| `--panel2` | `#1c1c1e` | Nested surface (rows, ghost buttons) |
| `--border` | `#2a2a2d` | All borders / dividers |
| `--text` | `#e7e7e9` | Primary text |
| `--muted` | `#8a8a90` | Secondary text, icons at rest |
| `--accent` | `#ef6b3b` | Primary accent (active nav, buttons, logo) |
| `--accent2` | `#f0823f` | Accent hover |
| `--radius` | `14px` | Card radius (controls use `10px`) |

Additional surfaces: input/codeblock background `#0e0e0f`; sidebar background `#101011`; nav hover `#1a1a1c`.

### Semantic colors

| Meaning | Color | Where |
|---------|-------|-------|
| Success / online | `#5fd0a8` (green) | `.badge.ok`, `.chip.round-robin`, "● online/live" |
| Info / fallback | `#6ea8fe` (blue) | `.chip.fallback` |
| Special / fusion | `#c792ea` (purple) | `.chip.fusion` |
| Danger / error | `#ff6b6b` (red) | quota bar ≥90%, HTTP ≥400 badges |
| Warning | `#f0c050` (amber) | retry-after / cautions |

**Routing strategy color map (consistent everywhere):** fallback = blue, round-robin = green, fusion = purple.

### Accent gradient

Logo / hero fills use `linear-gradient(135deg, var(--accent), #b5321e)`.

## Typography

- **Sans (UI):** `ui-sans-serif, system-ui, "Segoe UI", Roboto, Arial, sans-serif`. Base size `14px`.
- **Mono (values):** `ui-monospace, "SF Mono", Menlo, monospace` — applied to `input.input`, `code`, `.codeblock`.
- Headings: `h2` (topbar title) `18px`; `h3` (card title) `15px`, flex with a leading icon (`gap: 9px`).
- Small / meta text: `11–13px` in `--muted`.

## Spacing, radius, sizing

- Card radius `14px`; controls (button/input/chip/row) radius `10px`; pills/badges `999px`.
- Card padding `20px 22px`; content padding `24px 26px`; content `gap: 18px`; **content max-width `940px`**.
- Sidebar width `244px` (sticky, full-height, `overflow-y: auto`).
- Flex helpers: `.row` (horizontal, `gap: 10px`), `.grid` (vertical stack, `gap: 10px`).

## Layout

```
.layout (flex)
├── .sidebar  (244px, sticky, scrolls)   → brand + grouped nav
└── .main (flex column)
    ├── .topbar   → page title + subtitle, version badge, online badge
    └── .content  → stack of .card (max-width 940px, centered by left column)
```

- **Sidebar** groups: ungrouped top (Endpoint, Providers, Provider Nodes, Models, Combos, Playground),
  then `.navgroup` headers: **Media**, **Monitoring**, **Tools**, **System**.
- **Topbar** subtitle is the fixed tagline: *"Agentic orchestrator — fallback · round-robin · fusion"*.

## Components (CSS classes)

| Class | Purpose | Notes |
|-------|---------|-------|
| `.card` | Primary surface | `--panel` bg, border, `14px` radius |
| `.card h3` | Card header | flex + leading `icon()` |
| `.btn` | Primary button | accent bg, white, weight 600 |
| `.btn.ghost` | Secondary button | `--panel2` bg + border |
| `.btn:disabled` | Disabled | opacity `0.55` |
| `.input` | Text field | `#0e0e0f` bg; `textarea.input` resizes vertical; `select.input` min-width 280 |
| `.chip` | Tag / status pill | `+.fallback` blue, `+.round-robin` green, `+.fusion` purple |
| `.badge` | Topbar pill | `+.ok` green |
| `.navitem` | Sidebar link | `.active` = accent bg + accent text; `.ic` is the 18px icon slot |
| `.navgroup` | Sidebar section label | uppercase, `10.5px`, letter-spaced |
| `.modelrow` | List row | `--panel2` bg, space-between |
| `.codeblock` | Logs / preformatted | mono `12.5px`, scroll, max-height 360 |
| `.muted` | Secondary text | `--muted`, `13px` |

## Iconography

- One helper: `icon(name: &str) -> impl IntoView` in [`main.rs`](./crates/frontend/src/main.rs) returns an
  inline **Lucide** (MIT) SVG, `24` viewBox, `stroke="currentColor"`, `stroke-width="2"`, rendered at `18px`.
- Icons **inherit color** from context (muted at rest, accent when a nav item is active).
- **No emoji** anywhere user-facing. Status marks `✓ ✗ ● ○` in dynamic text are allowed (typographic, not pictographic).
- Current icon names: `plug, cpu, server, box, layers, play, mic, bar-chart, trending-up, target, terminal,
  languages, globe, wrench, shield, sparkles, key, dollar, settings, user, construction, activity, plus, network`.
- To add an icon: add a `name => "<path …>"` arm to `icon()` (paste Lucide inner SVG), then reference `icon("name")`.

## Brand

- **Name:** Mycelix. **Wordmark:** `M Y C` (spaced) in the sidebar `.brand h1`, subtitle `Aggregator · v<version>`.
- **Logo:** the `network` icon inside a `36px` gradient-orange rounded square (white stroke).
- Do not reintroduce "9Router" or older "M Y C — …" product naming in prose; the product is **Mycelix**.

## Navigation & routing

- **Hash routing**: each page is `/#/<key>` (e.g. `/#/providers`, `/#/usage`). The SPA is served at the site root.
- Active view is derived from `window.location.hash`; a `hashchange` listener keeps it in sync (back/forward + deep links work).
- Keys: `endpoint, providers, nodes, models, combos, playground, media, usage, quota, console, translator,
  proxy, cli, mitm, skills, keys, pricing, settings, profile`.

## Adding new UI — checklist

1. Reuse existing tokens/classes; **do not introduce new colors** unless adding a semantic meaning (document it here).
2. New page = add a nav `item(key, icon, label)`, a route arm in the `App` match, and a `title` arm.
3. Use `icon()` for all glyphs (no emoji). Add the Lucide path to `icon()` if missing.
4. Keep technical values in mono (`.input` / `code` / `.codeblock`); secondary text in `.muted`.
5. Respect the strategy color map (fallback=blue, round-robin=green, fusion=purple) and status colors above.
