# The connections file

All folders, connections and credential descriptions live in one TOML file. It holds no passwords, passphrases or keys, so it is safe to back up or keep in Git.

## Example

```toml
[[credential]]
id = "cred-ops"
name = "Ops team"
username = "ops"
kind = "private-key"
key_file = "~/.ssh/id_ops"

[[folder]]
id = "prod"
name = "Production"
credential = "cred-ops"
desktop.quality = "balanced"

[[connection]]
id = "bastion"
name = "bastion"
protocol = "ssh"
host = "bastion.example.net"

[[connection]]
id = "web-01"
parent = "prod"
name = "web-01"
protocol = "ssh"
host = "web-01.example.net"
jump = "bastion"
```

## Credentials

A credential says who to log in as and what kind of secret is used. The secret itself is kept in the operating system's keyring under the credential's `id`.

| Key | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique among credentials |
| `name` | yes | Label shown in the credential list |
| `username` | no | Login name |
| `domain` | no | Windows domain, for RDP |
| `kind` | yes | `password`, `private-key` or `ask` |
| `key_file` | no | Path to a private key file, for `private-key` |

With `ask`, nothing is stored and you are prompted on every connect.

## Folders

| Key | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique among folders |
| `name` | yes | Label shown in the tree |
| `parent` | no | Id of the enclosing folder; leave out for the top level |
| `icon` | no | Icon shown in the tree; see [Icons](#icons) |

## Connections

| Key | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique among connections |
| `name` | yes | Label shown in the tree |
| `parent` | no | Id of the enclosing folder |
| `protocol` | yes | `ssh`, `sftp`, `ftp`, `ftps`, `smb`, `nfs`, `wsl`, `local`, `rdp` or `vnc` |
| `host` | yes | Host name or IP address |
| `icon` | no | Icon shown in the tree; see [Icons](#icons) |

## Icons

`icon` takes one of these names:

`folder`, `server`, `database`, `monitor`, `terminal`, `laptop`, `cloud`, `globe`, `house`, `building`, `briefcase`, `network`, `router`, `cpu`, `hard-drive`, `box`, `lock`, `shield`, `key`, `wrench`, `flask`, `star`

Without `icon`, a folder shows the folder icon, an SSH connection a terminal, and an RDP or VNC connection a monitor. An unknown name is shown as the default too. Icons are not inherited.

## Inherited settings

These keys can be set on a folder or on a connection.

| Key | Meaning | When nothing sets it |
| --- | --- | --- |
| `port` | TCP port | 22 for SSH, 3389 for RDP, 5900 for VNC |
| `credential` | Id of a stored credential | No credential |
| `jump` | Id of a connection to tunnel through | Direct connection |
| `local.directory`, `local.arguments`, `local.environment` | For `local` shells: the start folder (`~` is home), a list of arguments, and a table of variables. `host` is the shell; `""` is your default shell | Home folder, no arguments, no variables |
| `desktop.quality` | For RDP and VNC: `lan`, `balanced` or `low-bandwidth` (files written before VNC call it `rdp.quality`, which still works) | `balanced` |

A connection uses its own value first. If it has none, it takes the value from its folder, then from that folder's parent, and so on up the tree.

In the example, `web-01` sets no credential, so it uses `cred-ops` from the `prod` folder.

## What is rejected

The file is checked when it is loaded and before it is saved. It is refused when:

- two folders, two connections or two credentials share an `id`
- a `credential` names a credential that does not exist
- a `parent` names a folder that does not exist
- a `jump` names a connection that does not exist
- a folder is inside itself, directly or through other folders
