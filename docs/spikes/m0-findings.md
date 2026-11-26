# M0 spike findings

13 Nov 2026. The prototype code was removed from the tree (it is in the git history); this page is what carries forward.

## Verdict

| Risk | Answer | How it was checked |
| --- | --- | --- |
| R1 File drop on Wayland | **Does not work** in iced 0.14. Works on X11 and XWayland. | Source, and confirmed on COSMIC (Pop!\_OS 24.04) |
| R2 IronRDP coverage | **Frame path works**, including NLA to Windows 11. Resize, clipboard, audio not exercised yet. | Automated test against xrdp; screenshots of xrdp and Windows 11 |
| R3 Terminal fed by SSH | **Works with our own thin widget.** iced\_term cannot be used. | Automated test and screenshot against OpenSSH |
| R4 System shortcuts on Wayland | **Not available** through winit. Ship the "send key combo" menu. | Source, and confirmed on COSMIC (Pop!\_OS 24.04) |
| New: R9 russh + IronRDP in one binary | **Resolved**: use the IronRDP sub-crates, not the `ironrdp` meta crate. | Both spikes build and pass in one workspace |

Toolkit: **stay on iced.** Nothing found is specific to iced; egui sits on the same winit and inherits R1 and R4 unchanged.

## R1 File drop on native Wayland

- iced 0.14 uses winit 0.30.13. Its Wayland backend has no data-device code, and both winit and iced document `FileDropped` as "Wayland: not implemented".
- winit 0.31 (beta 4 today) adds a Wayland `data_transfer` module and a reworked drop API. iced has not moved to it yet.
- X11 and Windows are implemented, so a drop works when the app runs through XWayland.

Options, in order of preference:

1. Wait for iced on winit 0.31. Until then run under XWayland by default on Linux (costs fractional scaling quality).
2. Write our own Wayland data-device module beside winit (the plan's fallback). Not prototyped.
3. "Upload" button plus paste-from-clipboard as the only path.

Confirmed on COSMIC (Pop!\_OS 24.04, native Wayland): dropping a file produced no hover or drop event at all.

Confirmed on the same machine through XWayland (`WAYLAND_DISPLAY= spike-wayland-probe`): hover and drop events both arrive with the full path. Shortcuts behave the same as on native Wayland.

Decision: on Linux, default to XWayland until iced ships on winit 0.31, then switch back to native Wayland.

Still to do by hand: KDE and GNOME.

## R2 IronRDP

- `spikes/rdp-frame` connects to xrdp over TLS, decodes bitmap updates into an RGBA buffer and shows them through iced's `image` widget. First real frame arrives in under 2 seconds.
- Built without `aws-lc` or cmake: rustls on `ring`, and our own no-op `NetworkClient` instead of sspi's reqwest one. Kerberos would need a real one.
- The spike uploads a full frame per update. The real view needs a texture updated by dirty rectangle (`GraphicsUpdate` already carries the rectangle).
- The blocking API on a thread was enough here; `ironrdp-tokio` is the fit for the planned one-task-per-session model.

- NLA (CredSSP with NTLM) to a Windows 11 VM works: `--nla --insecure` shows the desktop.
- The handshake to a real Windows host takes longer than a frame-loop read timeout allows, so connect and idle timeouts must be separate.

Not covered, still open: input, dynamic resize, clipboard, audio, gateway, drive redirection.

## R3 Terminal fed by an SSH channel

- iced\_term 0.8 creates a local PTY inside its backend (`tty::new`) with no way to inject a byte stream. It is also on alacritty\_terminal 0.25.
- `alacritty_terminal::Term` plus the `vte` parser accept bytes from anywhere. `spikes/term-ssh` feeds them from a russh channel and the remote prompt renders in an iced window.
- So the plan's "own thin widget" route is confirmed. The spike draws the grid as plain text; the real widget still needs colours, cursor, selection, scrollback and resize.
- The terminal's replies to the server (for example cursor position reports) arrive as `Event::PtyWrite` on the event listener. The spike drops them; the widget must write them back to the channel.

## R4 System shortcuts on Wayland

- Neither winit 0.30 nor 0.31 implements the `keyboard-shortcuts-inhibit` protocol, so Alt+Tab and Super stay with the compositor.
- Confirmed on COSMIC: the modifier press arrives, then the window loses focus; the `Tab` of Alt+Tab never reaches the app.
- Answer: ship the "send key combo" menu and document the limit. A custom protocol module could share the work with option 2 of R1.

## R9 russh and IronRDP in one binary (new, resolved)

- The `ironrdp` meta crate 0.17 (July) still pulls `ironrdp-connector 0.10`, whose `picky` and `sspi` pin `curve25519-dalek = 5.0.0-rc.1` exactly. russh 0.61 and later need the final `5.0.0`, so the two do not resolve together.
- Fix: skip the meta crate and depend on the sub-crates directly (`ironrdp-connector 0.12`, `ironrdp-session 0.13`, `ironrdp-graphics 0.11`, `ironrdp-pdu 0.11`, `ironrdp-blocking 0.12`). They are on final RustCrypto releases and share one lockfile with russh 0.65.
- Cost: four small API changes between connector 0.10 and 0.12. The sub-crates move faster than the meta crate, so pin them and wrap them behind our own module, as the plan already says.
- Plan change: the Tech stack table should list the IronRDP sub-crates, not "meta crate 0.17".

## Build pipeline

- `cargo xtask <lint | test | build linux | build windows>` runs the same steps locally and in `Jenkinsfile`, inside `ci/Dockerfile`.
- Verified: lint, tests and the Linux release build, on the host and inside the CI image.
- Verified: the Windows cross-build inside the CI image (`cargo-xwin`, `x86_64-pc-windows-msvc`). The `.exe` files have not been launched on Windows yet.
- Verified: the multibranch Jenkins job `remotething` on `forge`. Build 1 of `wp-m0-spikes` passed every stage in about 25 minutes with cold caches.
- russh defaults to `aws-lc-rs`, which needs cmake. The spikes select `ring`; it still compiles C and assembly, so the Windows cross-build needs clang in the image.
