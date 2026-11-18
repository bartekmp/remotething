# Changelog

What changed for people who use Remotething, newest first. New entries go
under "Unreleased"; `cargo xtask bump` moves them under the new version.

## Unreleased

## 0.70.0 - 2026-11-18

- Duplicate a connection from its right-click menu, then edit the copy.

## 0.69.1 - 2026-11-18

- The Linux packages carry the real logo (the menu and window icon were a placeholder), in every size, and the desktop entry matches the window.

## 0.69.0 - 2026-11-18

- A jump host with an "ask every time" credential now asks for its password when you connect, instead of refusing.

- The guide has screenshots, a shortcuts page and a troubleshooting page, and a new introduction.

## 0.68.0 - 2026-11-18

- High contrast colours in Settings, Tab and Shift+Tab move between the fields of forms, and an Accessibility chapter in the guide.

## 0.67.0 - 2026-11-18

- SSH remote forwards (`R:9000:localhost:3000`) and dynamic forwards (a SOCKS5 proxy, `D:1080`); the PuTTY import brings them over.

## 0.66.1 - 2026-11-18

- The SMB guide no longer says the administrative shares (`C$`) are listed; they are not.

## 0.66.0 - 2026-11-18

- NFS (experimental): browse and transfer files on an NFS 3 server. The server needs the `insecure` export option; see the NFS chapter of the guide.

## 0.65.4 - 2026-11-17

- The size shown while resizing is no longer shown over VNC.

## 0.65.3 - 2026-11-17

- The size shown while resizing appears only over SSH terminals, RDP and VNC.

## 0.65.2 - 2026-11-17

- The size shown while resizing is the remote desktop size that will be asked for, so it matches the status bar once it settles; a terminal shows only columns × lines.

## 0.65.1 - 2026-11-17

- The size shown while resizing sits in the top right corner of the remote view itself, below its buttons.

## 0.65.0 - 2026-11-17

- Resizing the window shows its size in the top right corner, and the columns × lines of a terminal in front.

## 0.64.1 - 2026-11-17

- The properties panel is a little wider and keeps its fields clear of the scrollbar.

## 0.64.0 - 2026-11-17

- Split view: a new tab takes the place of the pane that has the keyboard.
- Terminal themes: Alacritty's older YAML files can be imported.
- SCP file browsing shows modification times on servers with a GNU `ls`.

## 0.63.0 - 2026-11-17

- The compact file pane beside a terminal has icon buttons and no "This computer"; zoomed, the buttons have labels too.

## 0.62.0 - 2026-11-17

- The file pane beside a terminal shows only names and sizes, and has a Zoom button that fills the tab and shows permissions and dates.

## 0.61.2 - 2026-11-17

## 0.61.1 - 2026-11-17

- The window opens at the same size on screen at any interface size.

## 0.61.0 - 2026-11-17

- Settings → General: Interface size, to draw the whole window up to twice as big.

## 0.60.1 - 2026-11-17

- Ctrl+mouse wheel changes the terminal text size by one step per notch on X11, not two.

## 0.60.0 - 2026-11-17

- Terminal text size: Ctrl+Plus, Ctrl+Minus and Ctrl+mouse wheel.

## 0.59.0 - 2026-11-17

- Terminal font: Settings → Terminal takes the name of an installed monospace font.

## 0.58.0 - 2026-11-17

- Split view: Ctrl+Shift+R stacks or unstacks the panes, and a double-click on the divider makes them equal.

## 0.57.0 - 2026-11-17

- Split view: drag the divider to resize the panes, and stack them one above the other from a tab's menu.

## 0.56.0 - 2026-11-17

- Split view: two tabs side by side (Ctrl+Shift+S, or a tab's menu), Ctrl+Shift+O switches the keyboard between them.

## 0.55.0 - 2026-11-17

- Settings → Terminal: text size, and colour themes imported from Alacritty, kitty or Ghostty files, with a preview before applying.

## 0.54.0 - 2026-11-17

- File browser rows show permissions and modification time; a Details button hides them.

## 0.53.0 - 2026-11-17

- The file browser works on SSH servers that have no SFTP, using SCP and shell commands. Transfers there cannot resume.

## 0.52.0 - 2026-11-17

- F1 shows a list of the keyboard shortcuts.

## 0.51.0 - 2026-11-17

- The connection tree and the properties panel slide in and out, a carried tab lifts off the bar, and a dropped tab glows for a moment. Settings has a switch to turn animations off.

## 0.50.0 - 2026-11-17

- File browsers can be driven from the keyboard: arrows, Enter, Backspace, F2 to rename, Delete, F5, and Tab to switch between the two panes.

## 0.49.0 - 2026-11-17

- Command palette (Ctrl+Shift+P): jump to a connection or action by typing, with arrow keys and Enter. Ctrl+Tab and Ctrl+Shift+Tab cycle tabs, Alt+1 to Alt+9 pick one.

## 0.48.1 - 2026-11-17

- Copying from the server to this computer refuses a file or folder name that would lead outside the chosen local folder.

## 0.48.0 - 2026-11-16

- File browsers can show your own disk in a second pane beside the server's, with copy buttons between them.

## 0.47.0 - 2026-11-16

- A connection's properties can save a typed user name and password as a credential in one step, without going to the Credentials screen. The password goes to the system keyring and never to the connections file.

## 0.46.0 - 2026-11-16

- New protocol SMB: a file browser for Windows and Samba shares (SMB 2 and 3, pure Rust). The shares of the server are the top folders; uploads and downloads resume.

## 0.45.0 - 2026-11-16

- Windows: WSL distributions open as local terminals, as in Windows Terminal (`wsl.exe -d NAME`), with no addresses and no SSH server. The WSL import now lists every installed distribution and adds them as WSL connections. This replaces the address-based import of 0.44.0.

## 0.44.0 - 2026-11-16

- Windows: Settings → Import and export → Import from WSL finds the running WSL distributions and adds them as SSH connections in a WSL folder. Importing again updates their addresses.

## 0.43.0 - 2026-11-16

- Settings are grouped into pages: General, Import and export, Optional features and About.

## 0.42.1 - 2026-11-16

- Remote Desktop: files waiting to be pasted stay announced in the status strip ("3 files (12 MB) ready to paste") until they are pasted or the clipboard changes, instead of a message that fades after a few seconds.

## 0.42.0 - 2026-11-16

- Ctrl+F searches the connections; a middle click closes a tab; 32 more icons for folders and connections.
- Settings: choose the folders for screenshots, terminal recordings and desktop recordings. A folder that already exists keeps its permissions; the files are private either way.
- Remote Desktop: files copied in your file manager are offered to the remote desktop automatically; paste them there. This works on Wayland, where dropping files does not.
- Fixed: keys went to a remote desktop while the "close this tab?" question was open.

## 0.41.1 - 2026-11-16

- VNC: when the server sends an empty pointer, as Windows does on a machine without a mouse, your own arrow stays visible instead of no pointer at all.

## 0.41.0 - 2026-11-16

- VNC: servers that offer only the Tight security type now work with a password. A server that offers only its own user name and password method is reported with what it offered and how to fix it.

## 0.40.1 - 2026-11-16

- Remote Desktop: a progress bar over the status strip while files are received. Sending has none; Windows shows its own while it pastes.

## 0.40.0 - 2026-11-16

- Remote Desktop: copy files both ways. Send files with a button or by dropping them on the window and paste them on the remote desktop; copy files there and receive them into a folder. Nothing is ever replaced.

## 0.39.0 - 2026-11-16

- New protocols FTP and FTPS: the tab is a file browser. FTPS pins the server's certificate and asks before the password is sent. Uploads and downloads resume.

## 0.38.0 - 2026-11-16

- New protocol SFTP: a connection that opens only the file browser, with no shell. It signs in like SSH, jump hosts included.

## 0.37.1 - 2026-11-16

- Internal: the file pane works through a backend interface, ready for FTP.

## 0.37.0 - 2026-11-16

- File pane: drop files or folders on the window to upload them (not on a native Wayland window, where the toolkit gives no drop events).

## 0.36.0 - 2026-11-16

- File pane: a right-click menu on rows with rename, delete (asks first), new folder, copy path, preview and download.

## 0.35.0 - 2026-11-16

- File pane: preview a remote text file or picture without downloading it.

## 0.34.1 - 2026-11-16

- File pane: a folder download is refused when the server lists a file name that would write outside the chosen folder.

## 0.34.0 - 2026-11-16

- File pane: upload and download whole folders. Each file goes through the transfer list.

## 0.33.0 - 2026-11-16

- File pane: uploads and downloads go into a list with progress. Several can be queued, two run at once, each can be cancelled, and a failed or cancelled one can be retried from where it stopped.

## 0.32.2 - 2026-11-16

- Remote Desktop: when a broken connection is made again, the last picture stays on screen and recordings and the clipboard carry on. Resizes wait a little longer to settle, so fewer are sent.

## 0.32.1 - 2026-11-16

- Remote Desktop: a connection that breaks on the way, which Windows 11 sometimes causes when the desktop changes size, is made again by itself up to three times.

## 0.32.0 - 2026-11-15

- Recording of remote desktops (RDP and VNC) as a Motion JPEG video, an optional feature in Settings.

## 0.31.0 - 2026-11-15

- Screenshots, an optional feature in Settings: a PNG of an RDP or VNC desktop, or the text on an SSH terminal's screen.

## 0.30.1 - 2026-11-15

- Recordings are readable by their owner alone on Linux.

## 0.30.0 - 2026-11-15

- Optional features in Settings, all off at first. The first one records SSH terminals as asciicast files, with a red indicator while it runs.

## 0.29.1 - 2026-11-15

- Remote Desktop: a connection that the server drops half way through, as happens right after closing the previous session, is retried once.

## 0.29.0 - 2026-11-15

- Remote Desktop: pictures are shared between your clipboard and the remote one, as text already was.

## 0.28.0 - 2026-11-15

- VNC: quality presets; low bandwidth uses 16-bit colour. The quality setting now applies to both RDP and VNC (`desktop.quality` in the connections file; `rdp.quality` still loads).

## 0.27.0 - 2026-11-15

- VNC: tunnel through an SSH connection by choosing it as the jump host.

## 0.26.0 - 2026-11-15

- VNC: connections open a desktop tab with password sign-in, mouse, keyboard, text clipboard, pointer shapes and a connection details panel.

## 0.25.0 - 2026-11-15

- Groundwork for VNC: a new crate that connects to VNC servers with a password, and passes pictures, the pointer and input. Not usable from the app yet.

## 0.24.0 - 2026-11-15

- Remote Desktop: the pointer takes the shapes the remote machine asks for (text cursor, resize arrows, busy ring).

## 0.23.0 - 2026-11-15

- Remote Desktop: sharp picture and right-sized interface on screens with display scaling.

## 0.22.0 - 2026-11-15

- Remote Desktop: quality presets (best, balanced, low bandwidth) in the properties panel, and the RemoteFX codec, which cuts the picture data a lot.

## 0.21.0 - 2026-11-15

- A logo: the window and taskbar icon, and an About block with the version at the bottom of **Settings**.

## 0.20.0 - 2026-11-15

- Remote Desktop: text is shared between your clipboard and the remote one.

## 0.19.0 - 2026-11-15

- SSH: a status strip under the terminal with an (i) button that shows address, user, sign-in method, server software, host key, traffic and uptime.

## 0.18.0 - 2026-11-15

- Remote Desktop: a Ctrl+Alt+Del button in the status strip.

## 0.17.0 - 2026-11-15

- Remote Desktop: the desktop resizes with the tab.

## 0.16.0 - 2026-11-15

- Remote Desktop: a status strip under the desktop with an (i) button that shows address, security, codec, bandwidth, frame rate and uptime.

## 0.15.2 - 2026-11-15

- Remote Desktop: on machines without a usable GPU the desktop is drawn as a plain image instead of staying blank.

## 0.15.1 - 2026-11-15

- Remote Desktop: the picture is drawn through one GPU texture updated in place, to stop the blinking on some drivers.

## 0.15.0 - 2026-11-15

- Remote Desktop: RDP connections open in a tab with NLA sign-in, certificate pinning, mouse and keyboard.

## 0.14.0 - 2026-11-15

- Groundwork for RDP: a new crate that connects to Windows hosts with NLA, pins the server certificate and passes frames and input. Not usable from the app yet.

## 0.13.0 - 2026-11-15

- Terminal search: **Ctrl+Shift+F** finds text on the screen and in the scrollback.

## 0.12.0 - 2026-11-15

- Importing a Remotething file shows its connections first, so you can leave some out.

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
