# SSH

Double-click an SSH connection, or right-click it and choose **Connect**. A tab opens with a terminal.

## Signing in

What the app asks for depends on the connection's credential:

| Credential | What happens |
| --- | --- |
| None | The tab asks for a user name and a password |
| Password | The stored password is used; if none is stored, the tab asks for it |
| Private key | The key file is used; if it is encrypted and no passphrase is stored, the tab asks for it |
| Ask every time | The tab asks for the password on every connect |

A password typed into the tab is used for that session only. To store one, use the **Credentials** screen.

If the server refuses a password, the form comes back so you can try again. If it refuses a key, the tab says so; fix the credential and press **Reconnect**.

## Host keys

The first time you connect to a host, nothing is sent to it yet. The tab shows the fingerprint of the server's key:

1. Compare the fingerprint with the one the server's administrator gives you.
2. Press **Trust and connect**. To refuse, close the tab.

Trusted keys are kept in `known_hosts.toml`, beside the connections file, by host and port.

If a trusted host later shows a different key, the tab warns you before anything is sent. This happens when a server is reinstalled, and also when someone intercepts the connection. Trust the new key only if you know why it changed.

## The terminal

- 16, 256 and true colours, bold and underline. The colours follow the app's light or dark theme.
- The terminal resizes with the window, and the remote programs are told.
- The mouse wheel, or **Shift+Page Up** and **Shift+Page Down**, looks back through up to 10,000 lines. Typing returns to the newest line.
- Drag over text to select it. On Linux the selection is copied at once for middle-click paste; **Ctrl+Shift+C** copies it to the clipboard.
- **Ctrl+Shift+V** pastes from the clipboard. A middle click pastes what was last selected.
- A paste with a line break or another control character, or longer than 2,000 characters, asks first and shows its size and first lines. Enter pastes, Escape cancels. Line breaks are sent as Enter. Control characters other than tab and line breaks are removed from pasted text.
- Programs that ask for it (most shells and editors) are told that the text is a paste, so they do not run or indent it as if it was typed.
- A closed session shows why it ended and a **Reconnect** button. Closing the tab closes the connection.
- A keepalive is sent every 30 seconds so idle sessions are not dropped by firewalls.

## Files

A connected session has a **Browse files** button above the terminal. It opens a file pane on the right, beside the terminal, using the same SSH connection (SFTP), so there is no second sign-in.

| Action | How |
| --- | --- |
| Unfold or fold a folder | Click it |
| Go to the folder above | **Up** |
| Read the folders again | **Refresh** |
| Upload a file | **Upload**, then pick the file. It goes into the selected folder, beside the selected file, or into the top folder shown |
| Download a file | Select it, press **Download**, then choose where to save it |
| Close the pane | The **x** at its top |

- The pane starts in your home folder. Folders come first, then files, by name.
- An upload or download replaces a file of the same name without asking.
- One transfer runs at a time, and there is no progress bar yet; the result or the reason for a failure is shown at the bottom of the pane.

## Not available yet

| Feature | State |
| --- | --- |
| Jump hosts | A connection with a jump host says so and does not connect |
| ssh-agent, Pageant, `.ppk` keys, two-factor prompts | Planned |
| Search, selecting by word or line with a double or triple click | Planned |
| Port forwards | Planned |
| Files: progress and a transfer queue, rename, delete, new folder, preview, the wide view with permissions and dates, drag and drop | Planned |
| Host certificates | Not supported; the server must offer a plain host key |
