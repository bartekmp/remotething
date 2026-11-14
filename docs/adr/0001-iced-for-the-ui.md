# 1. iced for the UI

Date: 2026-11-13. Status: accepted.

## Decision

Use iced 0.14. Do not switch to egui.

## Why

The M0 spikes drew a live SSH terminal and an RDP desktop in iced without friction. The two gaps found, file drop and system shortcuts on native Wayland, come from winit, which egui also uses. Switching would fix neither.

## Consequences

- On Linux the app runs through XWayland by default until iced ships on winit 0.31, which adds Wayland file drop. See record 3.
- System shortcuts such as Alt+Tab cannot be forwarded on Wayland. The app offers a "send key combo" menu.
