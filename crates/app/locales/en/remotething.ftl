app-title = Remotething

tree-empty = No connections yet.

tabs-empty = Double-click a connection to open it, or press Ctrl+Shift+T for recent ones.

session-address = { $protocol } to { $host }, port { $port }
session-credential = Credential: { $credential }
session-no-credential = No credential
session-jump = Through jump host: { $jump }
session-not-implemented = Connecting is not available yet. Protocols arrive in the next releases.

protocol-ssh = SSH
protocol-rdp = RDP
protocol-vnc = VNC

error-load = The connections file could not be loaded:

tree-new-folder = New folder
tree-new-connection = New connection
tree-new-folder-name = New folder
tree-new-connection-name = New connection
tree-new-connection-host = host.example.net

properties-empty = Select a folder or connection to edit it.
properties-folder = Folder
properties-connection = Connection
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
credentials-close = Back to sessions
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
settings-close = Back to sessions
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
files-up = Up
files-refresh = Refresh
files-upload = Upload
files-download = Download
files-empty = This folder is empty.
files-loading = Loading…
files-working = Transferring…
files-uploaded = Uploaded { $file } ({ $size }).
files-downloaded = Downloaded to { $file } ({ $size }).

jump-needs-credential = The jump host { $name } needs a credential with a user name and either a stored password or a key file. A jump host cannot ask while connecting.
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
properties-forwards = Port forwards
properties-forwards-hint = 8080:db.internal:5432, 9000:localhost:9000
properties-error-forwards = Port forward { $text } is not local_port:host:port.

kind-interactive = Keyboard-interactive
credentials-interactive-note = For servers that ask for a password and a one-time code. The questions are shown on every connect. Nothing is stored here except the user name.
session-interactive-refused = The server did not accept the answers.
session-cancel = Cancel

removal-question = Remove the folder "{ $name }"?
removal-contents = It holds { $folders } folders and { $connections } connections. They are removed with it, and this cannot be undone.
removal-yes = Remove all
removal-no = Keep

preview-title = Import connections
preview-explain = The connections in the file. Tick the ones to add; their folders, credentials and jump hosts come with them. Nothing that exists is changed, and passwords are not in the file.
preview-none = The file holds no connections.
preview-import = Import { $count } connections
preview-all = Tick all
preview-none-button = Tick none

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
