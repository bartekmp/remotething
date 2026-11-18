# Installing

The pipeline builds the Windows zip for every merge to the main branch. Linux packages are built on demand with `cargo xtask build linux` then `cargo xtask package linux` (they land in `dist/`).

## Linux

| System | Package | Install |
| --- | --- | --- |
| Ubuntu or Kubuntu 26.04 or later | `remotething_<version>_amd64.deb` | `sudo apt install ./remotething_<version>_amd64.deb` |
| Fedora and other RPM systems with glibc 2.43 or later | `remotething-<version>.x86_64.rpm` | `sudo dnf install ./remotething-<version>.x86_64.rpm` |

Both add Remotething to the application menu. They need the glibc of the system they were built on (2.43, Ubuntu 26.04 or newer) until the pipeline builds them on an older base.

Without a Vulkan or OpenGL driver the app still runs, with slower software rendering.

## Windows

Unzip `remotething-<version>-windows-x86_64.zip` anywhere and run `remotething.exe`. Windows 10 and 11, 64-bit.

The program is not code-signed yet, so SmartScreen shows a warning the first time. Choose **More info**, then **Run anyway**.

## Check the installation

```sh
remotething --version
```

## Not available yet

An AppImage and a Windows installer are planned.
