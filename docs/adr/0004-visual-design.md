# 4. Visual design: floating panels, one accent, tokens in one file

Date: 2026-11-13. Status: accepted.

## Decision

The app looks like InteriorForge: rounded panels floating on a tinted background, pill-shaped secondary buttons, the Inter typeface, hairline borders, one blue accent, and a light and a dark theme.

Every colour, radius and size is defined in `crates/app/src/theme.rs`. Screens use its styles and never define their own.

## Why

A consistent look is cheap while there are six screens and expensive later. One file of tokens keeps light and dark in step and makes a change of look a one-file change.

## Consequences

- Inter is bundled in the binary (SIL Open Font License 1.1; licence text beside the font files).
- Icons come from the Lucide icon font (ISC licence), bundled the same way, so they take the colour of the text around them.
- Panels are solid, not blurred glass: iced's stock widgets cannot blur what is behind them.
- "Follow the system" asks the desktop for light or dark at start-up and follows later changes.
- Contrast is unit-tested: body text at least 7:1, secondary text and button labels at least 4.5:1, in both themes.
