# Starting the app

```sh
remotething
```

The window has three parts: the connection tree on the left, open tabs in the centre, and the properties of the selected item on the right.

## The tree

- Click a folder's arrow to fold or unfold it.
- Click a folder's name, or a connection, to select it.
- Double-click a connection to open it in a new tab.
- The button at the top of each side panel folds it to a narrow strip; press it again to bring the panel back. The app remembers this.
- **New folder** and **New connection** add an item inside the selected folder, or beside the selected connection. With nothing selected they go to the top level.

### Right-click menu

| Right-click on | Offers |
| --- | --- |
| Empty space | New folder, New connection (both at the top level) |
| A folder | New folder and New connection inside it, Rename, Change icon, Remove |
| A connection | Connect, Rename, Change icon, Remove |

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

## Editing

Select an item, change its fields in the properties panel and press **Save**. Every save is written to the connections file straight away.

- **Icon** picks the icon shown in the tree. The first one in the grid is the default for that kind of item.
- Leave **Port** empty to inherit it from the folder or use the protocol's default.
- **(inherit)** for credential and jump host means the value comes from the folder above.
- **Delete** removes the item. A folder must be empty first, and a connection that others use as their jump host cannot be removed.

If a change is refused, nothing is saved and the reason appears at the top of the window.

Connecting is not available yet. For now a tab shows what the connection resolves to: protocol, host, port, credential and jump host.

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

Press **Settings** under the tree toolbar. Changes are saved as you make them.

| Setting | Choices | Default |
| --- | --- | --- |
| Theme | Follow the system, Light, Dark | Follow the system |
| Language | Follow the system, or a shipped language | Follow the system |

A theme change applies at once. A language change applies the next time you start the app. Only English is shipped so far.

Settings are kept in `settings.toml`, in the same directory as the connections file.

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
