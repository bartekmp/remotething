# FTP and FTPS

FTP connections open as a file browser that fills the whole tab. There is no terminal.

| Protocol | What it protects |
| --- | --- |
| **FTPS** | The password and every file travel over TLS (explicit TLS: the connection starts plain and upgrades before the password is sent). Default port 21. |
| **FTP** | Nothing. The password and the files cross the network readable by anyone on the way. The strip under the browser says **Not encrypted** in red. Use it only on a network you trust. |

Choose the protocol in the connection's properties, or type `ftp://user@host` or `ftps://user@host` in the new-tab box.

## Signing in

Use a **password** credential, or **ask every time**. Keys, the agent and one-time codes are SSH things. For public servers type `anonymous` as the user name.

## FTPS certificates

FTPS servers mostly use self-signed certificates, so there is no authority to check them against. The first time, the tab stops **before the password is sent** and shows the certificate's SHA-256 fingerprint. Compare it with the one the server's owner gives you, then **Trust and connect**. The certificate is remembered per host and port. If it later differs, the tab says so in red and trusts the new one only if you press the button again.

## Files

The file actions are the same as for [SFTP](ssh.md#files): upload, download, folders, drag and drop (where the system supports it), preview, rename, delete, new folder, copy path, and the transfer queue with cancel and retry.

- **Resume.** A retry carries on from what is already there, if the server supports it (most do).
- **Each transfer has its own connection**, so two can run at once. Listing and other small commands share one connection, which is opened again if the server drops it.
- **Rename** refuses a name that is taken, even though some FTP servers would replace the file.
- **Sizes and dates** come from the server's listing. Servers that list in an unusual format may show some entries without a size.

## Not available yet

- Implicit FTPS (the old port 990 style).
- Active mode; the connection always uses passive mode.
- Permission bits in the listing.
