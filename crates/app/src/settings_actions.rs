//! What the window does when Settings, or one of the screens it leads to, asks for something.

use iced::Task;

use crate::app::{Centre, EXPORT_FILE_NAME, Message, State};
use crate::display;
use crate::file_dialog;
use crate::i18n::fl;
use crate::import_preview;
use crate::putty_import;
use crate::settings;
use crate::wsl_import;

pub(crate) fn on_settings_event(state: &mut State, event: settings::Event) -> Task<Message> {
    match event {
        settings::Event::Changed => {
            state.save_preferences();
            display::set_interface_percent(state.preferences.interface_scale);
            state.tabs.rescale_desktops();
        }
        settings::Event::Terminal(message) => return state.terminal_message(message),
        settings::Event::PageChanged(page) => state.settings_page = page,
        settings::Event::OpenRepository => crate::link::open(crate::link::REPOSITORY),
        settings::Event::PickFolder(kind) => {
            return Task::perform(
                file_dialog::pick_folder(fl!("settings-folder-pick")),
                move |folder| Message::FolderPicked(kind, folder),
            );
        }
        settings::Event::Export => {
            let dialog = file_dialog::save(fl!("settings-export"), EXPORT_FILE_NAME.to_string());
            return Task::perform(dialog, Message::ExportFilePicked);
        }
        settings::Event::ImportPutty => {
            let (screen, task) = putty_import::State::open();
            state.putty = screen;
            state.open_settings(Centre::Putty);
            return task.map(Message::Putty);
        }
        settings::Event::ImportWsl => {
            let (screen, task) = wsl_import::State::open();
            state.wsl = screen;
            state.open_settings(Centre::Wsl);
            return task.map(Message::Wsl);
        }
        settings::Event::Import => {
            let dialog = file_dialog::open(fl!("settings-import"));
            return Task::perform(dialog, Message::ImportFilePicked);
        }
    }
    Task::none()
}

/// The screen that previews an import file.
pub(crate) fn on_preview(state: &mut State, message: import_preview::Message) {
    let Some(screen) = &mut state.import else {
        return;
    };
    match import_preview::update(screen, message) {
        Some(import_preview::Event::Close) => {
            state.import = None;
            state.settings_screen = Centre::Settings;
        }
        Some(import_preview::Event::Apply(tree)) => {
            state.import = None;
            state.settings_screen = Centre::Settings;
            state.import_tree(tree);
        }
        None => {}
    }
}

/// The screen that imports PuTTY sessions.
pub(crate) fn on_putty(state: &mut State, message: putty_import::Message) -> Task<Message> {
    let (task, event) = putty_import::update(&mut state.putty, message);
    match event {
        Some(putty_import::Event::Close) => state.settings_screen = Centre::Settings,
        Some(putty_import::Event::Apply(plan, names)) => state.apply_putty(&plan, &names),
        None => {}
    }
    task.map(Message::Putty)
}

/// The screen that imports WSL distributions.
pub(crate) fn on_wsl(state: &mut State, message: wsl_import::Message) -> Task<Message> {
    let (task, event) = wsl_import::update(&mut state.wsl, message);
    match event {
        Some(wsl_import::Event::Close) => state.settings_screen = Centre::Settings,
        Some(wsl_import::Event::Apply(plan, names)) => state.apply_wsl(&plan, &names),
        None => {}
    }
    task.map(Message::Wsl)
}
