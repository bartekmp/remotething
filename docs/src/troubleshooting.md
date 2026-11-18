# Troubleshooting

## The app asks for a master password at start

No system keyring was found, so passwords go in an encrypted file instead. See [Without a keyring](starting.md#without-a-keyring). Start a keyring (GNOME Keyring or KWallet) to avoid it.

## "The host key changed"

The server shows a different key than the one you trusted. It happens after a reinstall, and also when someone intercepts the connection. Ask the server's administrator for the new fingerprint before you trust it. See [Host keys](ssh.md#host-keys).

## A password is refused again and again

The form comes back each time the server refuses it. Check the user name too: a typed user name is used for that session only. Store the pair in **Credentials** to avoid typing it. A Windows server that uses a Microsoft account needs that account's name and password.

## Port forward "cannot listen"

The local port is in use by another program. Pick another port in the connection's **Port forwards**; the rest of the forwards still open. See [Port forwards](ssh.md#port-forwards).

## A jump host fails

Each jump host needs its own credential (a stored password, a key file, an agent, or **Ask every time**), and its SSH server must allow forwarding (`AllowTcpForwarding yes`). The tab names the host that refused. See [Jump hosts](ssh.md#jump-hosts).

## RDP: the connection drops when the window resizes

Windows 11 sometimes drops a record when the desktop resizes. The app reconnects by itself, up to three times, and gets the same desktop back. See [Remote Desktop](rdp.md).

## VNC: the screen does not follow the window

VNC has no resize box; the picture keeps the server's size. A lock screen can look blank until you press a key.

## SMB: no shares are listed

Only ordinary shared folders are listed. Administrative shares such as `C$` are not, but you can still type a path like `/C$/Users` once connected. See [SMB](smb.md).

## NFS: the connection is refused

The server must export the folder with the `insecure` option, because the app does not use a privileged source port. See [NFS](nfs.md).

## Dropping files on the window does nothing (Linux)

On a native Wayland window the toolkit receives no file drops. The app runs through XWayland by default, where drops work. Use **Upload** instead if you run it natively.

## SmartScreen blocks the Windows program

The program is not code-signed yet. Choose **More info**, then **Run anyway**. See [Installing](installing.md).

## The window is slow without a graphics driver

Without a Vulkan or OpenGL driver the app falls back to software rendering. Install the Mesa drivers for your GPU.

## Still stuck

Run `remotething --version` and open an issue on GitHub with the version, what you did and what the tab said.
