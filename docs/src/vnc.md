# VNC

Double-click a VNC connection to open its desktop in a tab.

## The pointer

The pointer over the picture is the server's own, as it sends it, drawn at your pointer's position. When the server sends none or an empty one, for example a Windows machine with no mouse attached, your system's arrow is shown instead, so you can always see where you point.

## Signing in

| The connection has | What happens |
| --- | --- |
| No credential | The app connects without a password. If the server asks for one, a form appears |
| A password credential | The stored password is used. If the server refuses it, the form appears again |
| "Ask every time" | The form appears first |

The user name is not used: VNC has passwords only. A key or ssh-agent credential cannot be used with VNC.

## Not encrypted, unless tunnelled

The base VNC protocol sends the picture and your typing in the clear, and its password check is weak. Use it on a network you trust, or through an SSH tunnel.

To tunnel, choose an SSH connection as the **Jump host** in the properties panel of the VNC connection (or of its folder). The app signs in to that SSH server, as for [jump hosts](ssh.md#jump-hosts), and the VNC connection runs inside it. The host and port of the VNC connection are then as the SSH server sees them, so `localhost` means the SSH server itself. The server's key is asked about on first use, and the details panel shows the route.

## Using the desktop

- The picture is scaled to fit the tab. The desktop keeps the size the server gives it.
- The mouse works as on the remote machine: move, the three buttons and the wheel. The server's pointer picture is drawn, so it changes shape when the server says so.
- Keys are sent as the characters they make, so your keyboard layout decides what appears. Everything is sent except the app's own tab keys: **Ctrl+Shift+T**, **Ctrl+Shift+W**, **Ctrl+Shift+P** (palette), **Ctrl+Tab**, **Ctrl+Shift+Tab** and **Alt+1** to **Alt+9**.
- Text is shared between your clipboard and the server's, in both directions. The protocol carries only Latin-1 characters, so others become `?`. Your clipboard is looked at once per second, only while this tab is in front and connected.
- Closing the tab ends the session and asks first.
- A closed or lost session shows why, with a **Reconnect** button.

## Desktop size

If the server can change its desktop size, which most can (TigerVNC, wayvnc and others say so when they connect), the desktop follows the tab: about a second after you stop resizing the window or the tree, the app asks for a desktop of the tab's size. Nothing needs to be set. A server that cannot, or does not answer, keeps its size and the picture is scaled to fit.

## Quality

Choose **Remote desktop quality** in the properties panel of a connection, or of a folder for everything inside it. It is the same setting as for [Remote Desktop](rdp.md#quality). Without a choice a connection uses **Balanced**.

| Preset | Colours | Pointer |
| --- | --- | --- |
| Best picture, Balanced | 32 bit | The server's pointer picture is drawn |
| Low bandwidth | 16 bit, about half the data for pictures that do not compress | Your own pointer shows, because servers send pointer pictures in 32 bit only |

A change applies the next time you connect.

## Connection details

A strip under the desktop shows the host, the desktop size, the download and upload rates and frames per second. The **(i)** button opens a panel with the address, how the connection is secured, the picture encodings offered (ZRLE, Tight, CopyRect, Raw), the traffic totals and the time connected.

## Signing in

The app signs in with the plain VNC password, which is what almost every server offers. Servers that offer only the **Tight** security type (older TightVNC and some others) work too: the app does Tight's negotiation and then uses the password.

Some servers offer only their own sign-in method with a user name and password, for example one called `SRP6DUAL`. Those are not supported. The tab says what the server offered; turn on **plain VNC password authentication** in the server's settings and connect again.

## Not available yet

| Feature | State |
| --- | --- |
| View-only mode, Apple Remote Desktop sign-in, user name and password sign-in methods (SRP, MS-Logon) | Not planned for now |
