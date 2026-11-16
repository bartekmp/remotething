//! How the list of uploads and downloads is drawn, below the file tree.

use iced::alignment::Vertical;
use iced::widget::{button, column, container, progress_bar, row, scrollable, text};
use iced::{Element, Length};

use crate::files_pane::{Message, State, format_size};
use crate::i18n::fl;
use crate::theme;
use crate::transfer_queue::{Direction, Job, JobState};

/// The list is scrolled when it grows past this height.
const LIST_HEIGHT: f32 = 170.0;

pub(crate) fn view(state: &State) -> Option<Element<'_, Message>> {
    if state.queue.jobs.is_empty() {
        return None;
    }
    let any_over = state.queue.jobs.iter().any(|job| !job.is_active());
    let clear = button(text(fl!("files-clear")).size(theme::TEXT_SMALL))
        .style(theme::pill)
        .on_press_maybe(any_over.then_some(Message::ClearPressed));
    let header = row![
        text(fl!("files-transfers"))
            .size(theme::TEXT_SMALL)
            .style(theme::secondary_text)
            .width(Length::Fill),
        clear
    ]
    .align_y(Vertical::Center);
    let jobs = column(state.queue.jobs.iter().map(view_job)).spacing(theme::GAP);
    Some(
        column![header, container(scrollable(jobs)).max_height(LIST_HEIGHT)]
            .spacing(6)
            .into(),
    )
}

fn view_job(job: &Job) -> Element<'_, Message> {
    let arrow = match job.direction {
        Direction::Upload => "↑",
        Direction::Download => "↓",
    };
    let name = job.remote.rsplit('/').next().unwrap_or(&job.remote);
    let action = match &job.state {
        JobState::Waiting | JobState::Running(_) => {
            Some((fl!("files-cancel"), Message::CancelPressed(job.id)))
        }
        JobState::Failed(_) | JobState::Cancelled => {
            Some((fl!("files-retry"), Message::RetryPressed(job.id)))
        }
        JobState::Done(_) => None,
    };
    let mut title = row![
        text(format!("{arrow} {name}"))
            .size(theme::TEXT_BODY)
            .wrapping(text::Wrapping::None)
            .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(Vertical::Center);
    if let Some((label, message)) = action {
        title = title.push(
            button(text(label).size(theme::TEXT_SMALL))
                .style(theme::pill)
                .on_press(message),
        );
    }

    let detail: Element<'_, Message> = match &job.state {
        JobState::Waiting => small(fl!("files-transfer-waiting")),
        JobState::Running(progress) => {
            let share = if progress.total == 0 {
                0.0
            } else {
                progress.done as f32 / progress.total as f32
            };
            column![
                progress_bar(0.0..=1.0, share).girth(4),
                small(fl!(
                    "files-transfer-progress",
                    done = format_size(progress.done),
                    total = format_size(progress.total)
                )),
            ]
            .spacing(3)
            .into()
        }
        JobState::Done(bytes) => small(fl!("files-transfer-done", size = format_size(*bytes))),
        JobState::Failed(reason) => text(reason.as_str())
            .size(theme::TEXT_SMALL)
            .style(text::danger)
            .into(),
        JobState::Cancelled => small(fl!("files-transfer-cancelled")),
    };
    column![title, detail].spacing(3).into()
}

fn small(label: String) -> Element<'static, Message> {
    text(label)
        .size(theme::TEXT_SMALL)
        .style(theme::secondary_text)
        .into()
}
