# Starting the app

```sh
remotething
```

The window has two parts: the connection tree on the left and open tabs in the centre. A properties panel floats over the right side while you edit an item.

## The tree

- Click a folder's arrow to fold or unfold it.
- Click a folder's name, or a connection, to select it.
- Double-click a connection to open it in a new tab.
- The button at the top of the tree folds it to a narrow strip; press it again to bring it back. The app remembers this.
- **New folder** and **New connection** add an item inside the selected folder, or beside the selected connection. With nothing selected they go to the top level. The properties panel opens for the new item.

### Right-click menu

| Right-click on | Offers |
| --- | --- |
| Empty space | New folder, New connection (both at the top level) |
| A folder | New folder and New connection inside it, Edit, Rename, Change icon, Remove |
| A connection | Connect, Edit, Rename, Change icon, Remove |

- **Edit** opens the properties panel.
- **Rename** turns the name into a text field. Enter keeps the new name, Escape keeps the old one.
- **Change icon** shows the icon grid; the change is saved when you pick one.
- Escape or a click outside closes the menu.

### Drag and drop

Press on a folder's name or on a connection, drag, and let go:

| Dropped on | Result |
| --- | --- |
| A folder | Moves inside that folder |
| A connection | Moves into that connection's folder; a connection is placed just before it |
| Empty space | Moves to the top level |

A folder cannot be dropped into itself or into one of its own sub-folders. Folders cannot be reordered among themselves yet; they keep the order of the file.

## Tabs

| Action | How |
| --- | --- |
| New tab | **Ctrl+Shift+T**, or the **+** at the end of the tab bar. It lists your recent connections; type to search all of them, press Enter for the first match |
| Close the tab in front | **Ctrl+Shift+W**, or the **x** on the tab. **Ctrl+W** works too, except in a connected terminal, where the shell uses it to delete a word |
| Reorder | Drag a tab onto another tab |

### Connecting without saving

Type a connection string into the search field of a new tab and press Enter. The connection opens in that tab and is not added to the tree.

| You type | Connects to |
| --- | --- |
| `ssh ops@192.168.0.1` | SSH as `ops`, port 22 |
| `ssh ops@web-01 -p 2222` or `ops@web-01:2222` | SSH on port 2222 |
| `ssh://ops@web-01:2222` | The same, as a URL |
| `rdp://desk` or `vnc://tv:5901` | RDP or VNC (these do not connect yet) |

To keep it, right-click the tab and choose **Save this connection**. It is added at the top level, and the properties panel opens with name, host, protocol and port filled in. A typed user name is saved as an "ask every time" credential linked to the connection.

Right-clicking any tab also offers **Close**.

- Closing a tab with a connected session asks first. Enter closes, Escape keeps it open. Other tabs close at once.
- The recent list holds the last 20 connections and is kept in `state.toml`, beside the connections file.
- When the tabs do not fit on one line, the bar keeps one line and a **+N** button lists the rest. **Settings** can switch this to wrapping onto up to three lines, which then scroll.

## Editing

Right-click an item and choose **Edit**. Change its fields in the properties panel and press **Save**: the change is written to the connections file and the panel closes. The **X** at the top, or Escape, closes it without saving.

- **Icon** picks the icon shown in the tree. The first one in the grid is the default for that kind of item.
- Leave **Port** empty to inherit it from the folder or use the protocol's default.
- **(inherit)** for credential and jump host means the value comes from the folder above.
- **Delete** removes the item. A folder must be empty first, and a connection that others use as their jump host cannot be removed.

If a change is refused, nothing is saved, the panel stays open and the reason appears at the top of the window.

SSH connections open a terminal; see [SSH](ssh.md). RDP and VNC cannot connect yet: their tab shows what the connection resolves to.

## Credentials

Press **Credentials** under the tree toolbar. The screen replaces the tabs until you press **Back to sessions**.

- **New credential** adds one; fill in the form and press **Save**.
- **Kind** is a password, a private key (with an optional passphrase), or "ask every time".
- The password or passphrase goes to your system keyring, never to the connections file. Leave the field empty to keep the one already stored.
- Choosing "ask every time" removes any stored secret for that credential.
- A credential that a folder or connection still uses cannot be deleted.

The keyring is Secret Service on Linux (GNOME Keyring, KWallet) and Credential Manager on Windows. If the keyring is locked, your desktop asks you to unlock it; the app waits up to a minute for that.

### Without a keyring

If no keyring is found at start-up, the app asks for a master password instead:

- The first time, choose a master password and repeat it. Your passwords are then kept in `secrets.toml`, an encrypted file beside the connections file.
- On later starts, enter the master password to unlock that file.
- **Not now** skips this; passwords you save are then kept only until you quit.

The master password is never stored. If you forget it, the file cannot be opened; delete `secrets.toml` and enter your passwords again.

The file is encrypted with XChaCha20-Poly1305 under a key derived from the master password with Argon2id.

## Settings

Press **Settings** at the bottom of the connection tree; with the tree folded it is the round cog button at the bottom of the strip. Changes are saved as you make them.

| Setting | Choices | Default |
| --- | --- | --- |
| Theme | Follow the system, Light, Dark | Follow the system |
| Language | Follow the system, or a shipped language | Follow the system |
| When tabs do not fit on one line | Keep one line and list the rest under a button; use more lines | Keep one line |

A theme change applies at once. A language change applies the next time you start the app. Only English is shipped so far.

Settings are kept in `settings.toml`, in the same directory as the connections file.

### Importing PuTTY sessions

In **Settings**, press **Import from PuTTY**. The screen lists the sessions it found and what each would become; tick the ones you want and press **Import**.

| Where it looks | |
| --- | --- |
| Linux | `~/.putty/sessions`, one file per session |
| Windows | The registry key `HKEY_CURRENT_USER\Software\SimonTatham\PuTTY\Sessions`, read with `reg export` |
| Any system | **Choose a .reg file** reads a registry export, so Windows sessions can be imported on Linux |

- Sessions go into a new folder called **PuTTY**. "Default Settings" becomes that folder's default port and user name.
- Host, port and user name are carried over. Sessions with the same user and key share one credential.
- Not imported, and said so on the screen: sessions that are not SSH (telnet, serial, raw, rlogin), proxy settings, remote and dynamic port forwards, and colours. Local port forwards are imported. A `.ppk` key file is kept as the credential's key file and used as it is.
- Passwords are never imported; PuTTY does not store them.
- PuTTY's data is only read, never changed.

### Moving connections to another copy of the app

1. In **Settings**, press **Export connections** and choose where to save the file.
2. Copy the file to the other machine.
3. There, press **Import connections** in **Settings** and pick the file.

| | |
| --- | --- |
| In the file | Folders, connections, credential names, user names and key file paths, as JSON |
| Never in the file | Passwords, passphrases, the key files themselves, trusted host keys |
| Existing items | Never changed. An imported item whose id is already taken gets a new id, and what points at it follows |

After an import, enter the passwords again on the **Credentials** screen. Importing the same file twice adds everything twice.

## Where the connections file lives

| System | Path |
| --- | --- |
| Linux | `~/.config/remotething/connections.toml` |
| Windows | `%APPDATA%\remotething\connections.toml` |

If the file does not exist, the app starts with an empty tree. If it cannot be read, the app still starts and shows the reason at the top of the window.

To use another file, for example on a USB stick:

```sh
remotething --config /path/to/connections.toml
```

The file format is described in [The connections file](connections-file.md).
