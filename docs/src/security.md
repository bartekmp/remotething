# Security

Where your passwords are, and what protects them.

## Passwords

- A password or key passphrase is stored in the **system keyring** (Secret Service on Linux, Credential Manager on Windows). If there is no keyring, it goes to `secrets.toml`, encrypted with XChaCha20-Poly1305. The key comes from your master password through Argon2id (64 MiB, 3 passes for new files). Every write uses a new random nonce.
- The connections file, the export file and the settings never hold a password. The connections file lists hosts, user names and credential names, so it is readable by you alone on Linux.
- Wrong password and a tampered file look the same to the app: the file does not open.
- In memory, a password has no `Debug` or `Display`, so it cannot reach a log or an error message, and it is wiped when dropped. The app writes no log.
- The master password is never stored. Nothing recovers a forgotten one.

## Servers

- **SSH**: the host key is pinned the first time. The tab stops before any credential is sent, and again if the key changes.
- **RDP and FTPS**: the certificate is pinned the same way, by its SHA-256 fingerprint. The sign-in starts only after you trust it.
- **FTP and plain VNC** are not encrypted. The tab says so. Tunnel VNC through SSH or use it on a network you trust.

## What the app cannot protect against

- Someone who runs code as you while the app is running can read its memory and the unlocked keyring. A keyring is as strong as your desktop session.
- A weak master password. Argon2id slows guessing down; it does not stop it.
- Recordings and screenshots show whatever was on screen. They are readable by you alone on Linux, and **Also record what I type** is off by default because passwords are typed in terminals.
- Memory the operating system may write to swap or a crash dump.

## Reporting a problem

Use **Report a vulnerability** under the repository's Security tab on GitHub. Please do not open a public issue for one.
