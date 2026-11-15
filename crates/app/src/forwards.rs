//! The port forwards of a connected session: opening them and showing which
//! ones run.

use std::sync::{Arc, Mutex};

use iced::widget::{column, text};
use iced::{Element, Task};
use remotething_core::Forward;
use remotething_remote::ssh::{Connection, Forwarding};

use crate::i18n::fl;
use crate::session::Message;
use crate::theme;

/// The forwards of one session. They close when this is dropped, so they end
/// with the session.
#[derive(Default)]
pub struct Active {
    running: Vec<(Forward, Forwarding)>,
    /// One line for each forward that could not be opened.
    failed: Vec<String>,
}

/// Freshly opened forwards on their way to the window. They are taken out once.
#[derive(Clone)]
pub struct Started(Arc<Mutex<Option<Active>>>);

impl std::fmt::Debug for Started {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Started")
    }
}

impl Started {
    pub fn take(&self) -> Option<Active> {
        self.0.lock().ok()?.take()
    }
}

/// Opens every wanted forward on the connection. Nothing happens for none.
pub fn start(connection: Connection, wanted: Vec<Forward>) -> Task<Message> {
    if wanted.is_empty() {
        return Task::none();
    }
    Task::perform(
        async move {
            let mut active = Active::default();
            for forward in wanted {
                let opened = connection
                    .forward_local(forward.local_port, forward.host.clone(), forward.port)
                    .await;
                match opened {
                    Ok(running) => active.running.push((forward, running)),
                    Err(error) => active.failed.push(error.to_string()),
                }
            }
            Started(Arc::new(Mutex::new(Some(active))))
        },
        Message::ForwardsStarted,
    )
}

impl Active {
    /// The lines shown above the terminal, or `None` when there is nothing to say.
    pub fn view(&self) -> Option<Element<'_, Message>> {
        if self.running.is_empty() && self.failed.is_empty() {
            return None;
        }
        let mut lines = column![].spacing(2);
        if !self.running.is_empty() {
            let list: Vec<String> = self
                .running
                .iter()
                .map(|(forward, _)| forward.to_string())
                .collect();
            let line = fl!("forwards-active", list = list.join(", "));
            lines = lines.push(
                text(line)
                    .size(theme::TEXT_SMALL)
                    .style(theme::secondary_text),
            );
        }
        for problem in &self.failed {
            lines = lines.push(
                text(problem.as_str())
                    .size(theme::TEXT_SMALL)
                    .style(text::danger),
            );
        }
        Some(lines.into())
    }
}
