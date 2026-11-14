//! Needs the `ssh` service from `spikes/servers/compose.yml`.
//! Run with: cargo test -p spike-term-ssh -- --ignored

use std::time::Duration;

use spike_term_ssh::screen::Screen;
use spike_term_ssh::ssh::open_shell;

#[tokio::test]
#[ignore = "needs the Docker SSH server"]
async fn remote_shell_output_reaches_the_screen() {
    let mut shell = open_shell("127.0.0.1:2222", "spike", "spike", 80, 24, false)
        .await
        .expect("connect to the test SSH server");
    let mut screen = Screen::new(80, 24);

    // The arithmetic proves the remote shell ran it: "42" is never typed.
    shell
        .input
        .send(b"echo answer-$((40 + 2))\n".to_vec())
        .await
        .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !screen.text().contains("answer-42") {
        let bytes = tokio::time::timeout_at(deadline, shell.output.recv())
            .await
            .expect("timed out waiting for shell output")
            .expect("channel closed early");
        screen.feed(&bytes);
    }
}
