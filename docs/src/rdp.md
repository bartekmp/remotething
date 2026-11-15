# Remote Desktop

Double-click a connection whose protocol is **RDP**. A tab opens and the Windows desktop appears in it.

## Signing in

| Credential | What happens |
| --- | --- |
| None | The tab asks for a user name and a password |
| Password | The stored password is used; if none is stored, the tab asks for it |
| Ask every time | The tab asks for the password on every connect |

The credential's **Domain** field is sent with the user name. SSH-only credentials (key, ssh-agent, keyboard-interactive) cannot be used for RDP; the tab says so.

The connection uses Network Level Authentication (NLA), so the server checks the password before it shows a desktop. If the server refuses the password, the form comes back.

## Server certificates

Windows servers present their own certificate. The first time you connect to a host, the tab shows its fingerprint and asks you to trust it; nothing secret has been sent yet. The fingerprint is remembered per host and port in `known_hosts.toml` next to the connections file.

If the certificate later changes, the tab warns before sending anything. Trust the new one only if you expected the change, for example after a reinstall.

## Using the desktop

- The picture is scaled to fit the tab. The desktop is 1280 x 800 for now.
- The mouse works as on the remote machine: move, the three buttons and the wheel.
- Keys are sent by their position on the keyboard, so the remote keyboard layout decides which character appears. Everything is sent except **Ctrl+Shift+T** and **Ctrl+Shift+W**, which stay with the app for opening and closing tabs. **Ctrl+W** goes to the remote machine.
- Closing the tab ends the session and asks first.
- A closed or lost session shows why, with a **Reconnect** button.

## Connection details

A strip under the desktop shows the host, the desktop size and the current download rate and frames per second. The **(i)** button at its right end opens a panel with:

| Line | Meaning |
| --- | --- |
| Address | The IP address and port actually connected to |
| Security | How the connection is secured |
| Picture codec | How the server sends the picture. Only classic bitmap updates are used so far, so a busy screen takes more bandwidth than in other clients |
| Desktop size | Pixels of the remote desktop |
| Receiving, Received in total | Current rate and total of RDP data received, without TLS overhead |
| Frames per second | Pictures shown per second, at most 30 |
| Connected for | Time since the session started |

The rates appear after the first second and update once per second. The same strip will serve VNC.

## Not available yet

| Feature | State |
| --- | --- |
| Resizing the desktop with the window, scaling for HiDPI | Planned |
| Quality presets | Planned (the setting exists, it is not used yet) |
| Clipboard, Ctrl+Alt+Del button | Planned |
| Cursor shape from the remote machine | Planned: the local pointer is shown |
| Kerberos sign-in, RD Gateway, audio, drive redirection | Not planned for now |
