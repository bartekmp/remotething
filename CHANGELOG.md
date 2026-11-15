# Changelog

What changed for people who use Remotething, newest first. New entries go
under "Unreleased"; `cargo xtask bump` moves them under the new version.

## Unreleased

## 0.11.0 - 2026-11-15

- Folders and connections can be reordered with **Move up** and **Move down** in the tree's context menu.

## 0.10.1 - 2026-11-15

- Tab titles too long for the tab are cut with an ellipsis; the full title shows when you hover over the tab.

## 0.10.0 - 2026-11-15

- Terminal: double-click selects a word, triple-click a line.

## 0.9.0 - 2026-11-15

- Removing a folder with content now asks first and removes everything inside it.
- The properties panel no longer covers the message bar.

## 0.8.1 - 2026-11-15

- PuTTY import now brings over local port forwards.

## 0.8.0 - 2026-11-15

- PuTTY `.ppk` key files (versions 2 and 3, encrypted or not) work as private key credentials, including ones imported from PuTTY.

## 0.7.0 - 2026-11-15

- Two-factor sign-in: a new "Keyboard-interactive" credential shows the server's questions (password, one-time code) in the tab.

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
