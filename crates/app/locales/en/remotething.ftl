app-title = Remotething

tree-empty = No connections yet.

tabs-empty = Double-click a connection to open it, press Ctrl+Shift+P to go to one by name, or Ctrl+Shift+T for recent ones.


protocol-ssh = SSH
protocol-sftp = SFTP
protocol-ftp = FTP
protocol-ftps = FTPS
protocol-smb = SMB
protocol-nfs = NFS (experimental)
protocol-rdp = RDP
protocol-vnc = VNC

error-load = The connections file could not be loaded:

tree-new-folder = New folder
tree-new-connection = New connection
tree-new-folder-name = New folder
tree-new-connection-name = New connection
tree-new-connection-host = host.example.net

properties-empty = Select a folder or connection to edit it.
properties-name = Name
properties-host = Host
properties-protocol = Protocol
properties-port = Port
properties-port-inherit = inherited or default
properties-parent = Folder
properties-credential = Credential
properties-jump = Jump host
properties-top-level = (top level)
properties-inherit = (inherit)
properties-save = Save
properties-delete = Delete

properties-error-name = The name cannot be empty.
properties-error-host = The host cannot be empty.
properties-error-port = The port must be a number from 1 to 65535.

error-save = The connections file could not be saved:

tree-credentials = Credentials

credentials-title = Credentials
credentials-new = New credential
credentials-new-name = New credential
credentials-empty = Select a credential, or create one.
credentials-name = Name
credentials-username = User name
credentials-domain = Windows domain
credentials-kind = Kind
credentials-key-file = Private key file
credentials-password = Password
credentials-passphrase = Key passphrase
credentials-secret-keep = leave empty to keep the stored one
credentials-save = Save
credentials-delete = Delete
credentials-error-name = The name cannot be empty.

kind-password = Password
kind-private-key = Private key
kind-ask = Ask every time

error-secret = The secret could not be stored:

tree-settings = Settings

settings-title = Settings
settings-theme = Theme
settings-language = Language
settings-language-note = A language change takes effect the next time you start the app.
settings-system = Follow the system
theme-light = Light
theme-dark = Dark

error-settings-load = The settings file could not be read, so defaults are used:
error-settings-save = The settings could not be saved:

vault-title-create = Protect your passwords
vault-title-open = Unlock your passwords
vault-explain-create = No system keyring was found. Choose a master password and Remotething will keep your passwords in an encrypted file instead.
vault-explain-open = Enter your master password to unlock the encrypted passwords file.
vault-reason = Keyring problem: { $reason }
vault-password = Master password
vault-confirm = Repeat the master password
vault-create = Create
vault-open = Unlock
vault-skip = Not now
vault-working = Working…
vault-error-empty = The master password cannot be empty.
vault-error-mismatch = The two passwords are not the same.
vault-error-wrong = Wrong master password, or the file was altered.
vault-skipped = Passwords are kept only until you quit.

properties-icon = Icon
tree-title = Connections
properties-title = Properties

menu-connect = Connect
menu-duplicate = Duplicate
tree-copy-name = { $name } (copy)
menu-rename = Rename
menu-change-icon = Change icon
menu-move-up = Move up
menu-move-down = Move down
menu-remove = Remove

session-connecting = Connecting to { $host }…
session-username = User name
session-password = Password
session-passphrase = Key passphrase
session-connect = Connect
session-username-missing = Enter a user name.
session-refused = The server refused the sign-in. Check the user name and password.
session-key-locked = The key file is encrypted. Enter its passphrase.
session-secret-failed = The stored password could not be read:
session-no-key-file = This credential has no private key file. Set one on the Credentials screen.
session-host-key-new = This is the first connection to { $host }. Check the key fingerprint with the server's administrator before you trust it.
session-host-key-changed = The key of { $host } has changed since you last trusted it. If nobody reinstalled the server, someone may be intercepting the connection.
session-fingerprint = Fingerprint
session-trust = Trust and connect
session-trust-changed = Trust the new key and connect
session-trust-failed = The host key could not be saved:
session-closed = The session has ended.
session-closed-status = The session has ended (exit status { $status }).
session-reconnect = Reconnect
session-close = Close
session-key-refused = The server does not accept this key for this user.

error-known-hosts = The file of trusted host keys could not be read, so no host is trusted:
menu-edit = Edit

tabs-new = New tab
tabs-close-question = Close { $name }?
tabs-close-explain = The session is still connected. Closing the tab disconnects it.
tabs-close-yes = Close
tabs-close-no = Keep open

launcher-search = Search, or type ssh user@host
launcher-recent = Recent
launcher-matches = Matches
launcher-no-recent = No recent connections yet. Type to search all of them.
launcher-no-matches = No connection matches.

settings-tabs = When tabs do not fit on one line
settings-tabs-single-line = Keep one line and list the rest under a button
settings-tabs-wrap = Use more lines (up to three, then scroll)

settings-exchange = Move connections to another copy of the app
settings-export = Export connections
settings-import = Import connections
settings-exchange-note = The file holds folders, connections and credential names. Passwords and trusted host keys are never exported.
export-done = Connections exported to { $file }.
export-failed = The connections could not be exported:
import-done = Imported { $connections } connections, { $folders } folders and { $credentials } credentials. { $renamed } got a new id because theirs was taken.
import-failed = Nothing was imported:

launcher-quick-heading = Connection string
launcher-quick = Connect to { $target }
launcher-quick-note = not saved
tabs-menu-save = Save this connection
tabs-menu-close = Close

paste-question = Paste this into the terminal?
paste-size = { $lines } lines, { $characters } characters
paste-explain = Each line break is sent as Enter, so a shell runs the lines as commands.
paste-yes = Paste
paste-no = Cancel

files-browse = Browse files
files-title = Files
files-zoom = Zoom
files-unzoom = Unzoom
files-up = Up
files-refresh = Refresh
files-upload = Upload
files-download = Download
files-upload-folder = Upload folder
files-preview = Preview
files-copy-path = Copy path
files-rename = Rename
files-new-folder = New folder
files-delete = Delete
files-menu-ok = OK
files-rename-label = New name
files-new-folder-label = Name of the new folder
files-delete-question = Delete { $name }? This cannot be undone.
files-delete-folder-question = Delete the folder { $name }? It must be empty. This cannot be undone.
files-preview-back = Back
files-preview-cut = Only the first { $size } is shown.
files-preview-too-large = This picture is too large to preview. Download it instead.
files-preview-binary = This file is not text or a picture, so it cannot be previewed. Download it instead.
files-preview-broken = The picture could not be read: { $reason }
files-download-folder = Save the folder into
files-unsafe-name = The server listed a file with an unsafe name ({ $name }). Nothing was downloaded.
files-empty = This folder is empty.
files-loading = Loading…
files-transfers = Transfers
files-transfer-waiting = Waiting
files-transfer-progress = { $done } of { $total }
files-transfer-done = Done, { $size }
files-transfer-cancelled = Cancelled
files-cancel = Cancel
files-retry = Retry
files-clear = Clear

jump-needs-credential = The jump host { $name } needs a credential: a stored password, a key file, an agent, or "ask every time". Keyboard-interactive sign-in does not work on a jump host.
jump-ask-unsupported = The jump host { $name } asks for its password, which only SSH tabs can do. Store the password in its credential.
session-jump-password = Password for the jump host { $name }
jump-loop = The jump hosts form a loop at { $name }.

files-replace-question = { $file } already exists on the server. Replace it?
files-replace-yes = Replace
files-replace-no = Cancel

settings-import-putty = Import from PuTTY
putty-title = Import from PuTTY
putty-back = Back to settings
putty-explain = Sessions found in PuTTY. Tick the ones to import; they are added in a new folder called PuTTY. Passwords are not imported, because PuTTY does not store them.
putty-choose-file = Choose a .reg file
putty-import = Import { $count } sessions
putty-none = No PuTTY sessions were found.
putty-skipped = Not imported:
putty-done = Imported { $count } PuTTY sessions into the PuTTY folder.

kind-agent = ssh-agent
credentials-agent-note = The keys held by your running ssh-agent are used. Nothing is stored here except the user name.
session-agent-needs-user = The credential for this connection uses ssh-agent and needs a user name. Set one on the Credentials screen.
session-agent-refused = The server accepted none of the keys in your ssh-agent for this user.

forwards-active = Port forwards: { $list }
properties-remote-command = Command to run
properties-remote-command-hint = Empty: the login shell. For example tmux new -A -s main
properties-forwards = Port forwards
properties-forwards-hint = 8080:db.internal:5432, R:9000:localhost:3000, D:1080
properties-error-forwards = Port forward { $text } is not port:host:port, R:port:host:port or D:port.

kind-interactive = Keyboard-interactive
credentials-interactive-note = For servers that ask for a password and a one-time code. The questions are shown on every connect. Nothing is stored here except the user name.
session-interactive-refused = The server did not accept the answers.
session-cancel = Cancel

removal-folder = Remove the folder "{ $name }"?
removal-connection = Remove the connection "{ $name }"?
removal-credential = Remove the credential "{ $name }"?
removal-credential-note = Its saved password or key passphrase is deleted too, and this cannot be undone.
removal-contents = It holds { $folders } folders and { $connections } connections. They are removed with it, and this cannot be undone.
removal-yes = Remove
removal-no = Keep

preview-title = Import connections
preview-explain = The connections in the file. Tick the ones to add; their folders, credentials and jump hosts come with them. Nothing that exists is changed, and passwords are not in the file.
preview-none = The file holds no connections.
preview-import = Import { $count } connections
preview-all = Tick all
preview-none-button = Tick none

search-open = Search
search-hint = Find in terminal
search-count = { $current } of { $total }
search-none = No matches

desktop-credential-kind = This connection uses a credential that is for SSH. For Remote Desktop use a password credential, or ask every time.
desktop-certificate-new = The certificate of { $host } is not known yet. Check that this is the machine you mean to reach, then trust it.
desktop-certificate-changed = The certificate of { $host } has changed. The machine may have been reinstalled, or someone may be between you and it. Trust the new one only if you expected the change.

details-protocol = Protocol
details-host = Host
details-address = Address
details-security = Security
details-codec = Picture codec
details-size = Desktop size
details-rate = Receiving
details-total = Received in total
details-frames = Frames per second
details-uptime = Connected for

desktop-ctrl-alt-del = Ctrl+Alt+Del
send-keys-button = Send keys
send-keys-custom = Custom…
send-keys-key = Key, for example F4 or Tab
send-keys-capture = Read from keyboard
send-keys-capturing = Press the keys now…
send-keys-unknown = That is not a key name
send-keys-nothing = Nothing chosen yet
send-keys-back = Back
send-keys-send = Send

details-user = User name
details-method = Sign-in
details-method-password = Password
details-method-key = Key file
details-method-agent = ssh-agent
details-method-interactive = Keyboard-interactive
details-server = Server software
details-host-key = Host key type
details-fingerprint = Host key fingerprint
details-terminal = Terminal size
details-forwards = Port forwards
details-received = Received
details-sent = Sent
details-via = Through { $hosts }

about-check-updates = Check for updates
about-update-checking = Checking…
about-update-current = You have the latest version.
about-update-available = Version { $version } is available.
about-update-download = Download
about-update-failed = Could not check for updates: { $reason }
about-rights = © 2026 bartekmp. Free software under the GNU GPL, version 3.

properties-quality = Remote desktop quality
quality-lan = Best picture (fast network)
quality-balanced = Balanced
quality-low-bandwidth = Low bandwidth
details-quality = Quality preset

vnc-credential-kind = This connection uses a credential that is for SSH. For VNC use a password credential, or ask every time.
vnc-needs-password = The server asks for a password.
vnc-refused = The server refused the password.
details-encodings = Picture encodings
details-security-tunnel = VNC password, inside an SSH tunnel
details-colours = Colour depth

recording-start = Record
recording-stop = Stop recording
recording-failed = Recording could not start:
recording-stopped = Recording stopped:
details-recording = Recording to
settings-features = Optional features
settings-feature-console = Record SSH terminals
settings-feature-keystrokes = Also record what I type (passwords are typed too)
settings-feature-every = Record every SSH session from its start
settings-features-note = Terminals are recorded as asciicast files (play with asciinema), desktops as AVI video. A recording is always shown in red in the status strip while it runs.

screenshot-button = Screenshot
screenshot-saved = Saved { $name }
screenshot-failed = Screenshot failed:
settings-feature-screenshots = Take screenshots (a picture of a remote desktop, the text of a terminal)
settings-feature-desktop = Record remote desktops (RDP and VNC)
desktop-reconnecting = The connection to { $host } was lost. Connecting again…
desktop-reconnecting-short = Connection lost, connecting again…
ftp-credential-kind = This connection uses a credential that is for SSH. For FTP use a password credential, or ask every time.
ftp-not-encrypted = Not encrypted
ftp-encrypted = Encrypted (TLS)
rdpfiles-send = Send files…
rdpfiles-receive = Receive ({ $count })
rdpfiles-pick-send = Send files to the remote desktop
rdpfiles-pick-folder = Save the files into
rdpfiles-available = { $count ->
    [one] 1 file copied remotely
   *[other] { $count } files copied remotely
}
rdpfiles-ready = { $count ->
    [one] 1 file ({ $size }) ready to paste
   *[other] { $count } files ({ $size }) ready to paste
}
rdpfiles-progress-received = Receiving { $done } of { $total }
rdpfiles-progress-sent = Sending { $done } of { $total }
rdpfiles-finished-received = { $count ->
    [one] Received 1 item
   *[other] Received { $count } items
}
rdpfiles-finished-sent = { $count ->
    [one] Sent 1 item
   *[other] Sent { $count } items
}
tree-search-hint = Search connections…
tree-search-empty = No connection matches.
settings-folder-change = Change…
settings-folder-default = Default
settings-folder-pick = Choose a folder
settings-tab-general = General
settings-tab-exchange = Import and export
settings-tab-features = Optional features
settings-tab-about = About
settings-import-wsl = Import from WSL
wsl-title = Import from WSL
wsl-explain = The WSL distributions installed on this computer. Tick the ones to add; they go in a folder called WSL. A connection opens a terminal in the distribution the way Windows Terminal does, so it needs no address and no SSH server, and a stopped distribution starts when you connect. Nothing is imported twice.
wsl-looking = Looking for WSL distributions…
wsl-refresh = Look again
wsl-none = No WSL distributions are installed.
wsl-import = Import { $count } distributions
wsl-done = { $added ->
    [one] Added 1 distribution
   *[other] Added { $added } distributions
}, { $existing } already there.
wsl-windows-only = WSL exists only on Windows.
wsl-not-installed = Could not run wsl.exe. Is WSL installed?
wsl-running = running
wsl-stopped = stopped
protocol-wsl = WSL
wsl-details-distribution = Distribution
smb-label = SMB
nfs-label = NFS, not encrypted (experimental)
properties-save-password = Or save a password for this connection
properties-save-password-user = User name
properties-save-password-secret = Password
properties-save-password-note = Saved in the system keyring and added to the credentials, named after this connection.
files-title-local = This computer
files-local-show = This computer
files-local-hide = Hide this computer
files-copy-to-local = Copy →
files-copy-to-remote = ← Copy
files-copy-exists = { $file } already exists here; it was not replaced.
palette-search = Go to a connection or action
palette-nothing = Nothing matches
palette-new-tab = New tab
palette-credentials = Credentials
palette-settings = Settings
settings-animations = Animate panels and tabs
settings-high-contrast = High contrast colours
settings-interface-size = Interface size
shortcuts-title = Keyboard shortcuts
shortcuts-palette = Go to a connection or action
shortcuts-new-tab = New tab
shortcuts-close-tab = Close the tab in front
shortcuts-cycle = Next and previous tab
shortcuts-number = Tab by number; 9 is the last
shortcuts-search = Search the connections
shortcuts-files-move = Move in a file browser, open and fold folders
shortcuts-files-pane = Switch between the two file panes
shortcuts-files-act = Rename, delete, refresh in a file browser
shortcuts-help = Show or hide this list
files-details-show = Details
files-details-hide = Hide details
settings-tab-terminal = Terminal
terminal-font-size = Text size: { $size } pt
terminal-font-family = Font (Enter to apply)
terminal-font-family-hint = Noto Sans Mono (bundled)
terminal-themes = Colour themes
terminal-theme-default = Default
terminal-theme-import = Import theme file…
terminal-theme-apply = Use this theme
terminal-theme-delete = Delete
terminal-theme-imported = Imported { $name }.
terminal-theme-import-failed = Cannot import the theme: { $reason }
terminal-theme-too-big = the file is too big to be a theme
terminal-theme-note = Reads theme files of Alacritty (.toml), kitty (.conf) and Ghostty. A ✓ marks the theme in use; the box shows how a theme looks before you use it.
error-terminal-themes = Cannot read the terminal themes:
tabs-menu-split = Show beside the front tab
tabs-menu-unsplit = Back to one tab
shortcuts-split = Split the view, or go back to one tab
shortcuts-other-pane = Keyboard to the other pane
tabs-menu-stack = Stack the panes
tabs-menu-side-by-side = Put the panes side by side
shortcuts-rotate = Stack or unstack the split panes
shortcuts-text-size = Terminal text size (also Ctrl+mouse wheel)
protocol-local = Local shell
local-terminal-title = Terminal
local-details-shell = Shell
local-details-directory = Folder
properties-shell = Shell: a command or a path
properties-shell-detected = Fill in from the shells found here
properties-shell-default = Your default shell
properties-arguments = Arguments
properties-arguments-hint = -l --rcfile "my rc"
properties-directory = Start in
properties-directory-hint = Home folder
properties-environment = Variables
properties-environment-hint = LANG=pl_PL.UTF-8 EDITOR=vim
properties-error-arguments = The arguments have an opening quote with no closing one.
properties-error-environment = A variable must look like NAME=value, but this does not: { $text }
palette-local-terminal = New local terminal
shortcuts-local-terminal = New local terminal
launcher-local = New local terminal
launcher-local-note = your default shell
palette-local-terminal = New local terminal
shortcuts-local-terminal = New local terminal
launcher-local = New local terminal
launcher-local-note = your default shell
files-column-permissions = Permissions
files-column-modified = Modified
files-column-size = Size
theme-follow-terminal = Follow terminal theme
settings-feature-follow-terminal = Offer "Follow terminal theme" in the Theme list
settings-feature-follow-terminal-note = With a terminal theme chosen on the Terminal page, the window takes its colours. Without one it follows the system.
properties-terminal-theme = Terminal theme
session-secret-missing = No saved password was found for this credential. Type it, then save it again in Credentials if it should be remembered.
details-resizing = Desktop follows the tab
details-resizing-yes = Yes
details-resizing-no = No, the server cannot resize
