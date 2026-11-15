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

- The remote desktop follows the size of the tab: a moment after you stop resizing the window or the tree, the server is asked for a desktop of that size. It starts at 1280 x 800 and is asked to resize about two seconds after connecting. Until the new size arrives the picture is scaled to fit.
- Screens with display scaling show the desktop at one remote pixel per logical pixel, so text is as small as on a 100 % screen.
- The mouse works as on the remote machine: move, the three buttons and the wheel.
- Keys are sent by their position on the keyboard, so the remote keyboard layout decides which character appears. Everything is sent except **Ctrl+Shift+T** and **Ctrl+Shift+W**, which stay with the app for opening and closing tabs. **Ctrl+W** goes to the remote machine.
- **Clipboard:** text is shared both ways. Text you copy on the remote machine appears on your clipboard, and text on your clipboard can be pasted there. Your clipboard is looked at once per second, and only while this tab is in front and connected; then its text is offered to the server. Images and files are not shared yet.
- **Ctrl+Alt+Del** cannot be typed, because your own system takes it. The **Ctrl+Alt+Del** button in the strip under the desktop sends it to the remote machine and opens Windows' security screen. Press Escape or choose Cancel there to leave it.
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
| Scaling for HiDPI screens | Planned |
| Quality presets | Planned (the setting exists, it is not used yet) |
| Clipboard images and files | Planned (text works) |
| Cursor shape from the remote machine | Planned: the local pointer is shown |
| Kerberos sign-in, RD Gateway, audio, drive redirection | Not planned for now |
