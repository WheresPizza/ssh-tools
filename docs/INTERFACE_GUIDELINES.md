# SSH GUI — Terminal workspace

A compact desktop interface inspired by terminal tools: monospace typography, clear boundaries, quiet surfaces and explicit text. This is the project's own design system, implemented in `src/styles/globals.css`; no external font or stylesheet is loaded.

## Typography and density

- System monospace stack: SF Mono, Menlo, Consolas, Liberation Mono. Body and controls: 13px; metadata: 12px; card/dialog titles: 14px; page titles: 16px. Keep the root at 16px and support browser zoom.
- Use sentence case. Uppercase is reserved for short navigation group labels. Paths, aliases and fingerprints remain selectable; wrap long values in details and provide a title when truncating list rows.
- Base spacing: 4px. Page padding: 20px; cards: 12–16px; related controls: 8px; sections: 16–20px. Do not shrink hit areas to match the smaller type.

## Color and hierarchy

- Follow the system light/dark preference. Graphite surfaces in dark mode; neutral paper surfaces in light mode. Teal is the action/selection accent.
- Use semantic tokens, not literal colors: `primary`, `muted-foreground`, `success`, `warning`, `destructive`. Status always includes words or an icon, never color alone.
- One main action per task uses `btn btn-primary`; secondary actions use `btn`; destructive actions use `btn btn-danger`. Never make deletion the default focused action.
- Borders separate surfaces. Avoid gradients, glow, decorative shadows and oversized empty-state illustrations. Card radius: 6px; controls: 4px.

## Components

- Buttons: shared `.btn`, minimum 32px high. `.btn-compact`: 28px for row actions only. Preserve explicit labels; allow action groups to wrap when needed.
- Inputs/selects: `.input-field`, minimum 34px high, visible label, same background and border. Help and validation text sit directly beneath the field.
- Navigation: 15px Lucide outline icons, 36px rows, active left border and `aria-current="page"`. Avoid emoji because platform rendering and color vary.
- Dialogs: title → explanation/form → actions; constrain height and allow scrolling. Keep Escape/cancel, keyboard focus and focus trapping where provided.
- Notifications: neutral surface with semantic border and text, `status` for routine outcomes and `alert` for errors. A labelled close action remains available.
- Technical status must be factual. Do not show a connected/secure indicator just because the app is open. Agent constraints describe the last requested policy, not a policy the agent cannot report.

## Accessibility and review

- Visible 2px focus outline with 3px offset on all interactive controls. Do not remove focus styling.
- Keep metadata readable; no reduced-opacity text for meaningful content. Disabled controls may be dimmed.
- Honor reduced motion. Support light and dark native controls with `color-scheme`.
- Review populated and empty screens, forms, destructive confirmation, long paths, keyboard navigation and the minimum supported window size. Run `npm test` and `npm run build` after shared styling changes.

## Extending the system

Reuse the shared tokens and component classes before introducing a new variant. If a new variant is required, document its purpose here and define it centrally rather than copying a utility chain into each screen.

## Information architecture

See [screen ownership](SCREEN_OWNERSHIP.md) before adding navigation items or actions. Each entity has one editing owner; contextual links may lead to a shared workflow.

## Brand assets

The application mark combines a terminal prompt and a key on a graphite tile. Its editable source is `public/app-icon.svg`; the menu-bar template is `src-tauri/icons/tray-source.svg`. Run `npm run icons` to regenerate native assets. Use the same application mark in README and the webview favicon. Keep the tray mark monochrome so macOS can adapt it to the menu-bar appearance.
