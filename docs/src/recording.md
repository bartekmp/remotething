# Recording

Optional features that save what happens in a session. Each is off until you switch it on in **Settings**, under **Optional features**. A feature that is off adds no button, runs no code and writes no file.

## Screenshots

1. In **Settings**, tick **Take screenshots**.
2. In an RDP or VNC tab press **Screenshot** in the strip under the desktop; in an SSH tab, the same button.

| Tab | What is saved |
| --- | --- |
| RDP, VNC | A PNG of the whole remote desktop, at its real size |
| SSH | A text file with the lines on the terminal's screen. Scrolled-back history is not included |

Files go to the `screenshots` folder beside the connections file, named after the connection and the time in UTC, for example `win11-20261115-231500.png`. The strip shows the file name for a few seconds. As with recordings, the folder and files are readable by you alone on Linux. The app cannot tell where a password field is, so a screenshot shows whatever was on screen.

## Recording an SSH terminal

1. In **Settings**, tick **Record SSH terminals**.
2. In an SSH tab, press **Record** in the strip under the terminal. A red dot and the elapsed time show while it runs.
3. Press **Stop recording** to end it. The (i) panel shows where the file is.

Two more switches appear once recording is on:

| Switch | Effect |
| --- | --- |
| Record every SSH session from its start | Every SSH tab starts recording by itself |
| Also record what I type | The keys you type are kept too. **Off by default, because passwords are typed in terminals** |

### The files

Recordings are in the `recordings` folder beside the connections file, named after the connection and the time in UTC, for example `web-01-20261115-231500.cast`. A second recording in the same second gets a number.

They use the asciicast v2 format. Play one with [asciinema](https://asciinema.org):

```sh
asciinema play web-01-20261115-231500.cast
```

On Linux the folder and the files can be read by you alone, because a recording holds whatever was on the screen. On Windows they get the permissions of your user folder.

Every event is written to the file as it happens, so a recording that is cut off by a crash or a lost connection still plays up to that point. Nothing is uploaded, and the app never opens a recording on its own.

If writing fails, for example because the disk is full, the recording stops and the strip says why.

## Coming next

Recording of remote desktops is planned as a further feature in the same place.
