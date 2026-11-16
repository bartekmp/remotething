# Recording

Optional features that save what happens in a session. Each is off until you switch it on in **Settings**, under **Optional features**. A feature that is off adds no button, runs no code and writes no file.

## Screenshots

1. In **Settings**, tick **Take screenshots**.
2. In an RDP or VNC tab press **Screenshot** in the strip under the desktop; in an SSH tab, the same button.

| Tab | What is saved |
| --- | --- |
| RDP, VNC | A PNG of the whole remote desktop, at its real size |
| SSH | A text file with the lines on the terminal's screen. Scrolled-back history is not included |

Files go to the `screenshots` folder beside the connections file, or to the folder you chose in Settings (**Change…** under the feature; **Default** goes back), named after the connection and the time in UTC, for example `win11-20261115-231500.png`. The strip shows the file name for a few seconds. As with recordings, the folder and files are readable by you alone on Linux. The app cannot tell where a password field is, so a screenshot shows whatever was on screen.

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

Recordings are in the `recordings` folder beside the connections file, or in the folder you chose in Settings for terminal recordings, named after the connection and the time in UTC, for example `web-01-20261115-231500.cast`. A second recording in the same second gets a number.

They use the asciicast v2 format. Play one with [asciinema](https://asciinema.org):

```sh
asciinema play web-01-20261115-231500.cast
```

On Linux the folder and the files can be read by you alone, because a recording holds whatever was on the screen. On Windows they get the permissions of your user folder.

Every event is written to the file as it happens, so a recording that is cut off by a crash or a lost connection still plays up to that point. Nothing is uploaded, and the app never opens a recording on its own.

If writing fails, for example because the disk is full, the recording stops and the strip says why.

## Recording a remote desktop

1. In **Settings**, tick **Record remote desktops (RDP and VNC)**.
2. In an RDP or VNC tab press **Record** in the strip under the desktop. A red dot and the elapsed time show while it runs.
3. Press **Stop recording**. The (i) panel shows the file while it is written.

The video is an AVI file of Motion JPEG pictures, ten per second, in the `recordings` folder beside the connections file, or in the folder you chose in Settings for desktop recordings (terminals and desktops have a folder each), for example `win11-20261115-231500.avi`. Any player opens it (`ffplay`, VLC, mpv). Nothing in the picture is hidden: passwords that show on the remote screen show in the video.

- The file is finished when you press **Stop recording** or close the tab, so it plays right away.
- When the remote desktop changes size, or the file reaches 1 GiB, the recording goes on in a new file with the next number.
- Pictures that did not change take no space. A busy screen takes about one megabyte per second; an idle one almost nothing.
- Recording goes on while another tab is in front.
- The file is readable by you alone on Linux.

## Credits

The JPEG encoder is based in part on the work of the Independent JPEG Group.

## Choosing the folders

In **Settings**, each ticked feature shows its folder with **Change…**. Screenshots, terminal recordings and desktop recordings can each go somewhere else. A folder that does not exist is made, private to you; a folder that already exists, such as `~/Videos`, keeps its own permissions. The files in it are still readable by you alone.
