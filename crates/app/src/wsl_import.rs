//! The screen that previews an import of the running WSL distributions: what was found,
//! what each becomes, what the user still has to do in it. Nothing is written until the user
//! presses Import.

use std::collections::HashSet;

use iced::widget::{button, checkbox, column, row, scrollable, text};
use iced::{Element, Length, Task};
use remotething_core::{WslDistro, WslPlan, plan_wsl};

use crate::i18n::fl;
use crate::theme;
use crate::wsl_source;

#[derive(Debug, Default)]
pub struct State {
    /// True while `wsl.exe` is being asked.
    pub(crate) loading: bool,
    /// What was found, once it is known.
    pub(crate) plan: Option<WslPlan>,
    /// The distributions ticked for import, by name.
    pub(crate) chosen: HashSet<String>,
    /// Why nothing could be found.
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The running distributions were looked up, or that failed.
    Found(Result<Vec<WslDistro>, String>),
    Toggled(String, bool),
    RefreshPressed,
    ImportPressed,
    BackPressed,
}

/// What the screen asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Add the ticked distributions to the tree.
    Apply(WslPlan, Vec<String>),
    Close,
}

impl State {
    /// A screen that is about to look for running distributions.
    pub fn open() -> (Self, Task<Message>) {
        let mut state = Self::default();
        let task = state.look();
        (state, task)
    }

    /// Starts looking, in the background.
    fn look(&mut self) -> Task<Message> {
        self.loading = true;
        self.error = None;
        Task::perform(
            async {
                tokio::task::spawn_blocking(wsl_source::running_distributions)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|found| found)
            },
            Message::Found,
        )
    }

    fn show(&mut self, found: Result<Vec<WslDistro>, String>) {
        self.loading = false;
        match found {
            Ok(distros) => {
                let plan = plan_wsl(&distros);
                self.chosen = plan.connections.iter().map(|c| c.name.clone()).collect();
                self.plan = Some(plan);
                self.error = None;
            }
            Err(reason) => {
                self.plan = None;
                self.chosen.clear();
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
        Message::RefreshPressed => {
            if !state.loading {
                return (state.look(), None);
            }
        }
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
        text(fl!("wsl-title"))
            .size(theme::TEXT_DISPLAY)
            .width(Length::Fill),
        button(text(fl!("putty-back")))
            .style(theme::pill)
            .on_press(Message::BackPressed),
    ];
    let mut body = column![text(fl!("wsl-explain")).style(theme::secondary_text)].spacing(10);

    if state.loading {
        body = body.push(text(fl!("wsl-looking")).style(theme::secondary_text));
    }
    if let Some(reason) = &state.error {
        body = body.push(text(reason.as_str()).style(text::danger));
    }
    if let Some(plan) = &state.plan {
        body = body.push(found_list(state, plan));
    }
    let import = button(text(fl!("wsl-import", count = state.chosen.len())))
        .style(theme::primary)
        .on_press_maybe((!state.chosen.is_empty()).then_some(Message::ImportPressed));
    let refresh = button(text(fl!("wsl-refresh")))
        .style(theme::pill)
        .on_press_maybe((!state.loading).then_some(Message::RefreshPressed));
    body = body.push(row![import, refresh].spacing(8));

    column![header, scrollable(body).height(Length::Fill)]
        .spacing(16)
        .into()
}

fn found_list<'a>(state: &'a State, plan: &'a WslPlan) -> Element<'a, Message> {
    let mut list = column![].spacing(8);
    if plan.connections.is_empty() && plan.skipped.is_empty() {
        list = list.push(text(fl!("wsl-none")).style(theme::secondary_text));
    }
    for distro in &plan.connections {
        let target = match &distro.user {
            Some(user) => format!("{user}@{}", distro.address),
            None => distro.address.clone(),
        };
        let name = distro.name.clone();
        let label = format!("{}  ({target})", distro.name);
        let mut entry = column![
            checkbox(state.chosen.contains(&distro.name))
                .label(label)
                .on_toggle(move |on| Message::Toggled(name.clone(), on))
        ]
        .spacing(2);
        for note in &distro.notes {
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
#[path = "wsl_import_tests.rs"]
mod tests;
