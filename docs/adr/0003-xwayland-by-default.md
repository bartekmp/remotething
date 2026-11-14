# 3. XWayland by default on Linux, for now

Date: 2026-11-13. Status: accepted, temporary.

## Decision

On Linux, start through XWayland unless the user opts into native Wayland.

## Why

Dropping a file onto a session is a headline feature. Tested on COSMIC (Pop!\_OS 24.04): native Wayland delivers no drop event with iced 0.14; XWayland delivers hover and drop with the full path.

## Consequences

- Fractional scaling is less sharp under XWayland.
- Revisit when iced moves to winit 0.31. Still to test on KDE and GNOME.
