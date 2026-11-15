# VNC

Double-click a VNC connection to open its desktop in a tab.

## Signing in

| The connection has | What happens |
| --- | --- |
| No credential | The app connects without a password. If the server asks for one, a form appears |
| A password credential | The stored password is used. If the server refuses it, the form appears again |
| "Ask every time" | The form appears first |

The user name is not used: VNC has passwords only. A key or ssh-agent credential cannot be used with VNC.

## Not encrypted

The base VNC protocol sends the picture and your typing in the clear, and its password check is weak. Use it on a network you trust, or through an SSH tunnel.

## Using the desktop

- The picture is scaled to fit the tab. The desktop keeps the size the server gives it.
- The mouse works as on the remote machine: move, the three buttons and the wheel. The server's pointer picture is drawn, so it changes shape when the server says so.
- Keys are sent as the characters they make, so your keyboard layout decides what appears. Everything is sent except **Ctrl+Shift+T** and **Ctrl+Shift+W**, which stay with the app for opening and closing tabs.
- Text is shared between your clipboard and the server's, in both directions. The protocol carries only Latin-1 characters, so others become `?`. Your clipboard is looked at once per second, only while this tab is in front and connected.
- Closing the tab ends the session and asks first.
- A closed or lost session shows why, with a **Reconnect** button.

## Connection details

A strip under the desktop shows the host, the desktop size, the download and upload rates and frames per second. The **(i)** button opens a panel with the address, how the connection is secured, the picture encodings offered (ZRLE, CopyRect, Raw), the traffic totals and the time connected.

## Not available yet

| Feature | State |
| --- | --- |
| Tunnel through an SSH connection | Planned |
| Quality presets, the Tight encoding | Planned |
| Resizing the desktop with the tab | Planned (servers that allow it) |
| View-only mode, Apple Remote Desktop sign-in | Not planned for now |
