//! The screen that previews a PuTTY import: what was found, what each
//! session becomes, what cannot be carried over. Nothing is written until
//! the user presses Import.

use std::collections::HashSet;
use std::path::PathBuf;

use iced::widget::{button, checkbox, column, row, scrollable, text};
use iced::{Element, Length, Task};
use remotething_core::{ImportPlan, PuttySession, plan_putty_import};

use crate::file_dialog;
use crate::i18n::fl;
use crate::putty_source;
use crate::theme;

#[derive(Debug, Default)]
pub struct State {
    /// What was found, once it has been read.
    pub(crate) plan: Option<ImportPlan>,
    /// The sessions ticked for import, by name.
    pub(crate) chosen: HashSet<String>,
    /// Why the sessions could not be read.
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// PuTTY's own sessions were read, or could not be.
    Found(Result<Vec<PuttySession>, String>),
    Toggled(String, bool),
    RegFilePressed,
    /// The `.reg` file chosen, or `None` when the dialog was cancelled.
    RegFilePicked(Option<PathBuf>),
    ImportPressed,
    BackPressed,
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Add the ticked sessions to the tree.
    Apply(ImportPlan, Vec<String>),
    Close,
}

impl State {
    /// A screen that is about to read PuTTY's own sessions.
    pub fn open() -> (Self, Task<Message>) {
        let task = Task::perform(async { putty_source::default_sessions() }, Message::Found);
        (Self::default(), task)
    }

    fn show(&mut self, found: Result<Vec<PuttySession>, String>) {
        match found {
            Ok(sessions) => {
                let plan = plan_putty_import(&sessions);
                self.chosen = plan.connections.iter().map(|c| c.name.clone()).collect();
                self.plan = Some(plan);
                self.error = None;
            }
            Err(reason) => {
                self.plan = None;
                self.error = Some(reason);
            }
        }
    }
}

pub fn update(state: &mut State, message: Message) -> (Task<Message>, Option<Event>) {
    match message {
        Message::Found(found) => state.show(found),
        Message::Toggled(name, ticked) => {
            if ticked {
                state.chosen.insert(name);
            } else {
                state.chosen.remove(&name);
            }
        }
        Message::RegFilePressed => {
            let dialog = file_dialog::open_any(fl!("putty-choose-file"));
            return (Task::perform(dialog, Message::RegFilePicked), None);
        }
        Message::RegFilePicked(Some(file)) => {
            let read = async move { putty_source::sessions_in_reg_file(&file) };
            return (Task::perform(read, Message::Found), None);
        }
        Message::RegFilePicked(None) => {}
        Message::ImportPressed => {
            if let Some(plan) = &state.plan {
                let mut names: Vec<String> = state.chosen.iter().cloned().collect();
                names.sort();
                return (Task::none(), Some(Event::Apply(plan.clone(), names)));
            }
        }
        Message::BackPressed => return (Task::none(), Some(Event::Close)),
    }
    (Task::none(), None)
}

pub fn view(state: &State) -> Element<'_, Message> {
    let header = row![
        text(fl!("putty-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("putty-back")))
            .style(theme::pill)
            .on_press(Message::BackPressed),
    ];
    let mut body = column![text(fl!("putty-explain")).style(theme::secondary_text)].spacing(10);

    if let Some(reason) = &state.error {
        body = body.push(text(reason.as_str()).style(text::danger));
    }
    if let Some(plan) = &state.plan {
        body = body.push(found_list(state, plan));
    }
    let file_button = button(text(fl!("putty-choose-file")))
        .style(theme::pill)
        .on_press(Message::RegFilePressed);
    let import = button(text(fl!("putty-import", count = state.chosen.len())))
        .style(theme::primary)
        .on_press_maybe((!state.chosen.is_empty()).then_some(Message::ImportPressed));
    body = body.push(row![import, file_button].spacing(8));

    column![header, scrollable(body).height(Length::Fill)]
        .spacing(16)
        .into()
}

fn found_list<'a>(state: &'a State, plan: &'a ImportPlan) -> Element<'a, Message> {
    let mut list = column![].spacing(8);
    if plan.connections.is_empty() && plan.skipped.is_empty() {
        list = list.push(text(fl!("putty-none")).style(theme::secondary_text));
    }
    for connection in &plan.connections {
        let target = match (&connection.username, connection.port) {
            (Some(user), Some(port)) => format!("{user}@{}:{port}", connection.host),
            (Some(user), None) => format!("{user}@{}", connection.host),
            (None, Some(port)) => format!("{}:{port}", connection.host),
            (None, None) => connection.host.clone(),
        };
        let name = connection.name.clone();
        let ticked = state.chosen.contains(&connection.name);
        let label = format!("{}  ({target})", connection.name);
        let mut entry = column![
            checkbox(ticked)
                .style(crate::theme::check)
                .label(label)
                .on_toggle(move |on| Message::Toggled(name.clone(), on))
        ]
        .spacing(2);
        for note in &connection.notes {
            entry = entry.push(
                text(format!("  {note}"))
                    .size(theme::TEXT_SMALL)
                    .style(theme::secondary_text),
            );
        }
        list = list.push(entry);
    }
    if !plan.skipped.is_empty() {
        list = list.push(text(fl!("putty-skipped")).size(theme::TEXT_SMALL));
        for skipped in &plan.skipped {
            list = list.push(
                text(format!("  {}: {}", skipped.name, skipped.reason))
                    .size(theme::TEXT_SMALL)
                    .style(theme::secondary_text),
            );
        }
    }
    list.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(name: &str, protocol: &str) -> PuttySession {
        PuttySession {
            name: name.to_string(),
            host: Some(format!("{name}.example.net")),
            protocol: Some(protocol.to_string()),
            ..PuttySession::default()
        }
    }

    fn found() -> State {
        let mut state = State::default();
        let sessions = vec![
            session("a", "ssh"),
            session("b", "ssh"),
            session("c", "telnet"),
        ];
        let _ = update(&mut state, Message::Found(Ok(sessions)));
        state
    }

    #[test]
    fn everything_importable_starts_ticked_and_the_rest_is_listed_as_skipped() {
        let state = found();

        assert_eq!(state.chosen.len(), 2);
        assert_eq!(state.plan.as_ref().unwrap().skipped.len(), 1);
    }

    #[test]
    fn unticking_leaves_a_session_out_of_the_import() {
        let mut state = found();

        let _ = update(&mut state, Message::Toggled("a".to_string(), false));
        let (_, event) = update(&mut state, Message::ImportPressed);

        match event {
            Some(Event::Apply(_, names)) => assert_eq!(names, ["b"]),
            other => panic!("expected an import of b, got {other:?}"),
        }
    }

    #[test]
    fn nothing_is_applied_until_import_is_pressed() {
        let mut state = found();

        let (_, event) = update(&mut state, Message::Toggled("a".to_string(), true));

        assert_eq!(event, None);
    }

    #[test]
    fn a_source_that_cannot_be_read_shows_why_and_clears_the_list() {
        let mut state = found();

        let _ = update(
            &mut state,
            Message::Found(Err("no such folder".to_string())),
        );

        assert_eq!(state.error.as_deref(), Some("no such folder"));
        assert!(state.plan.is_none());
    }

    #[test]
    fn back_closes_the_screen() {
        let mut state = found();

        let (_, event) = update(&mut state, Message::BackPressed);

        assert_eq!(event, Some(Event::Close));
    }
}
