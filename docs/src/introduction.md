<p align="center"><img src="logo.svg" alt="Remotething logo" width="96"></p>

# Introduction

Remotething is a desktop app for remote connections: SSH terminals, RDP and VNC desktops, and file transfers over SFTP, SCP, FTP, SMB and NFS, organised in one tree. It runs on Linux and Windows.

![A terminal session beside the connection tree](images/terminal-light.png)

## Five minutes

1. **New connection** (left panel). Give it a name, a host and a protocol, then **Save**.
2. Double-click it. The first time, the tab shows the server's fingerprint; compare it and press **Trust and connect**.
3. Type the password when asked, or store it once in **Credentials** so the app never asks again.
4. Press **Ctrl+Shift+P** to jump to any connection by name, or **F1** to see every shortcut.

Your connections live in one readable file, so they can be backed up or kept in version control; see [The connections file](connections-file.md). Passwords never go in it: they are kept in the system keyring.

## Where to go next

| I want to | Read |
| --- | --- |
| Organise connections in folders, split the view, import PuTTY sessions | [Starting the app](starting.md) |
| Use a terminal, jump hosts, port forwards, SFTP | [SSH](ssh.md) |
| Open a Windows desktop | [Remote Desktop](rdp.md) and [VNC](vnc.md) |
| Move files to a server | [FTP and FTPS](ftp.md), [SMB](smb.md), [NFS](nfs.md) |
| Take screenshots or record a session | [Recording](recording.md) |
| Find a key or a setting | [Keyboard shortcuts](shortcuts.md), [Accessibility](accessibility.md) |
| Fix something that does not work | [Troubleshooting](troubleshooting.md) |
