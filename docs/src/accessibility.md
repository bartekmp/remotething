# Accessibility

## What you can change

All of these are in **Settings**.

| Setting | What it does |
| --- | --- |
| Theme | Light, dark, follow the system, or a named colour scheme (Dracula, Catppuccin, Gruvbox, Nord, Solarized, Monokai). High contrast overrides the scheme and keeps only its light or dark |
| **High contrast colours** | Black on white (or white on black) with full-strength borders. Text and borders reach a contrast of at least 7:1 (WCAG AAA) |
| Interface size | Draws the whole window from 50% to 300% |
| **Animate panels and tabs** | Switch off for panels that appear at once and no glow on a dropped tab |
| Terminal text size and colour theme | 10 to 24 pt, and any imported theme; see [SSH](ssh.md) |

Even without high contrast, ordinary text is at least 7:1 and secondary text 4.5:1 on its panel in both themes. A test checks this for every release.

## Keyboard

Everything in the window can be done from the keyboard:

- **Ctrl+Shift+P** opens the command palette: type a few letters of a connection or an action and press Enter. It reaches every action in the app.
- **F1** lists all shortcuts, also over a remote desktop.
- **Ctrl+Tab** and **Ctrl+Shift+Tab** change tabs, **Alt+1** to **Alt+9** jump to one.
- **Tab** moves between fields in forms; **Enter** confirms and **Esc** closes a panel or box.
- In the file browser, the arrow keys move the selection and **Tab** switches panes.

## Screen readers

Not supported yet. The toolkit the app is built on (iced) does not expose the window to the system's accessibility services, so a screen reader sees one blank window. The command palette and the keyboard shortcuts work, but their results cannot be read aloud. This will change when the toolkit gains support.
