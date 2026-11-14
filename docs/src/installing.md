# Installing

The pipeline builds the Windows zip for every change. Linux packages are built on demand for now and will be published with tagged releases.

## Linux

| System | Package | Install |
| --- | --- | --- |
| Debian 12 or later, Ubuntu 22.04 or later | `remotething_<version>_amd64.deb` | `sudo apt install ./remotething_<version>_amd64.deb` |
| Fedora and other RPM systems | `remotething-<version>.x86_64.rpm` | `sudo dnf install ./remotething-<version>.x86_64.rpm` |

Both add Remotething to the application menu. They need glibc 2.35 or later.

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
