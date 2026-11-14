app-title = Remotething

tree-empty = No connections yet.

tabs-empty = Double-click a connection to open it.

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
menu-remove = Remove
