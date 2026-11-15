# Changelog

What changed for people who use Remotething, newest first. New entries go
under "Unreleased"; `cargo xtask bump` moves them under the new version.

## Unreleased

## 0.6.0 - 2026-11-15

- Local port forwards (`-L`) per connection or folder, opened with the SSH session.

## 0.5.0 - 2026-11-14

- Sign in with keys held by ssh-agent (OpenSSH agent or Pageant on Windows) through a new "ssh-agent" credential kind, also for jump hosts.

## 0.4.0 - 2026-11-14

- Import PuTTY sessions from Settings, with a preview: from `~/.putty/sessions` on Linux, the registry on Windows, or a `.reg` file.

## 0.3.1 - 2026-11-14

- Uploading to a file pane folder now asks before replacing a file of the same name.

## 0.3.0 - 2026-11-14

- Connect through one or more jump hosts, with each host's key checked and named.

## 0.2.0 - 2026-11-14

- Version shown in Settings; the version now goes up with every feature and fix.
- Connection tree with folders, inherited settings, icons, right-click menus, in-place rename and drag and drop.
- Credentials kept in the system keyring, or in an encrypted file behind a master password.
- SSH sessions in tabs: host-key trust on first use, password and key-file sign-in, a terminal with colours, scrollback, selection, copy and guarded paste.
- Tabs: new-tab page with recent connections and typed connection strings, close question, reordering, overflow handling.
- File pane on SSH sessions: folder tree, upload and download over SFTP.
- Export and import of connections as JSON.
- Light and dark themes, following the system by default.

## 0.1.0

Nothing released; the starting point of the version count.
