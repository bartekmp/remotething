# Installing

The pipeline builds the Windows zip for every merge to the main branch. Linux packages are built on demand with `cargo xtask build linux` then `cargo xtask package linux` (they land in `dist/`).

## Linux

| System | Package | Install |
| --- | --- | --- |
| Ubuntu or Kubuntu 26.04 or later | `remotething_<version>_amd64.deb` | `sudo apt install ./remotething_<version>_amd64.deb` |
| Fedora and other RPM systems with glibc 2.43 or later | `remotething-<version>.x86_64.rpm` | `sudo dnf install ./remotething-<version>.x86_64.rpm` |

Both add Remotething to the application menu. They need the glibc of the system they were built on (2.43, Ubuntu 26.04 or newer) until the pipeline builds them on an older base.

### Flatpak

`cargo xtask package flatpak` builds `remotething-<version>.flatpak` in `dist/`, on any distribution that has Flatpak, with no glibc requirement. It needs `flatpak-builder` (or the `org.flatpak.Builder` Flatpak) and the freedesktop 25.08 SDK with its Rust extension. Install the bundle with:

```sh
flatpak install --user dist/remotething-<version>.flatpak
```

The Flatpak may use the network, the window system and GPU, the desktop keyring, ssh-agent and your home folder (the "This computer" pane, key files and `~/.ssh/known_hosts`). Its settings are kept apart from a `.deb` install, in `~/.var/app/io.github.bartekmp.Remotething/config`. It is not on Flathub.

Without a Vulkan or OpenGL driver the app still runs, with slower software rendering.

## Windows

Windows 10 and 11, 64-bit. Releases on GitHub have two downloads:

- `remotething-<version>-windows-x86_64-setup.exe` installs for your user only, without administrator rights, and adds a Start menu entry. `setup.exe /S` installs silently. Uninstall it from **Installed apps**; your connections and settings stay.
- `remotething-<version>-windows-x86_64.zip` holds the program alone. Unzip it anywhere and run `remotething.exe`.

The program is not code-signed yet, so SmartScreen shows a warning the first time. Choose **More info**, then **Run anyway**.

## Check the installation

```sh
remotething --version
```

## Not available yet

An AppImage is planned.
