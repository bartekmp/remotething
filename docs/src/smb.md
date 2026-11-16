# SMB (Windows and Samba shares)

An SMB connection opens as a file browser that fills the whole tab. There is no terminal. It speaks SMB 2 and 3, the protocol of Windows file sharing, Samba, and most NAS devices. SMB 1 is not supported: it is obsolete and insecure.

Choose the protocol **SMB** in the connection's properties, or type `smb://host` in the new-tab box. The host is the server's name or address; the default port is 445.

## Shares

The top of the browser lists the **shares** the server offers, as folders. Open one to browse it. Printer shares and the administration pipe (`IPC$`) are left out. The administrative disk shares of a Windows machine (`C$` and the like) show too, and open only for an administrator.

You cannot create, rename or delete a share here, only what is inside one.

## Signing in

Use a **password** credential, or **ask every time**. If the account belongs to a domain, put the domain in the credential; a local account on the server leaves it empty. Kerberos is not offered in the app yet, only the password sign-in (NTLM).

A wrong user name or password asks again with a note. Messages are signed, and encrypted when the server requires it.

## Files

The file actions are the same as for [SFTP](ssh.md#files): upload, download, folders, drag and drop (where the system supports it), preview, rename, delete, new folder, copy path, and the transfer queue with cancel and retry.

- **Resume.** A retry carries on from what is already there.
- **Several transfers at once** share the one connection, so they do not each sign in again.
- **Rename** never replaces a name that is taken; the server refuses it and the reason is shown.
- **Names.** Characters Windows does not allow in names (`?`, `*`, `"`, `:`, `<`, `>`, `\`, `|`) are mapped the way macOS does, so a file keeps its name here and in Finder.

## Not available yet

- Kerberos sign-in and DFS namespaces that need it.
- Moving a file to another share by renaming it.
- Looking at file permissions.
