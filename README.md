<p align="center"><img src="assets/logo/remotething.svg" alt="Remotething logo" width="96"></p>

# Remotething

Remote connections in one tree: SSH terminals, RDP and VNC desktops, and file transfers over SFTP, SCP, FTP, SMB and NFS. Linux and Windows. In development, not yet 1.0.

<p align="center"><img src="docs/src/images/terminal-light.png" alt="An SSH session beside the connection tree" width="720"></p>

## What it does

- **One tree** of folders and connections, with settings inherited from folders, readable in one TOML file you can back up or version.
- **SSH terminal** with search, colour themes, jump hosts, local, remote and dynamic (SOCKS5) port forwards, host-key pinning, ssh-agent, keyboard-interactive and key files.
- **Files** beside any terminal or on their own: SFTP, SCP, FTP/FTPS, SMB and NFS, with a transfer queue, two panes, preview and drag and drop.
- **RDP and VNC** desktops with resize, clipboard (text, images, files over RDP) and quality presets.
- **Command palette** (Ctrl+Shift+P), tabs you can split, a keyboard route to everything.
- **Passwords in the system keyring**, never in the connections file; an encrypted file when there is no keyring.
- **Screenshots and recordings** of terminals and desktops, off until you switch them on.
- **Light, dark and high-contrast** themes, interface scaling, and imports of PuTTY and WSL.

<p align="center">
<img src="docs/src/images/files-light.png" alt="The file browser" width="48%">
<img src="docs/src/images/terminal-dark.png" alt="The dark theme" width="48%">
</p>

## Install

Build and run from source:

```sh
cargo run --release -p remotething
```

CI builds a Windows zip for every merge to `main`. The program is not code-signed yet; see the [installing guide](docs/src/installing.md).

## Documentation

- [User guide](docs/src/introduction.md) (`mdbook build docs` builds it as a site)
- [Keyboard shortcuts](docs/src/shortcuts.md) and [troubleshooting](docs/src/troubleshooting.md)
- [Architecture](ARCHITECTURE.md) and [contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md)

## License

GNU General Public License, version 3 only; see [LICENSE](LICENSE). Bundled fonts and icons keep their own licences, listed in [NOTICE](NOTICE).
