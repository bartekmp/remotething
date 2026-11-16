//! How the tabs are drawn: the bar, the content of the tab in front, and
//! what floats over them (overflow list, tab menu, close question).

use iced::alignment::Vertical;
use iced::widget::{
    button, column, container, mouse_area, opaque, pin, responsive, row, scrollable, space, stack,
    text, tooltip,
};
use iced::{Element, Length, Point};
use remotething_core::Store;

use crate::desktop_view;
use crate::ftp_view;
use crate::i18n::fl;
use crate::icons;
use crate::launcher;
use crate::preferences::TabLayout;
use crate::recent::Recent;
use crate::session_view;
use crate::tab_bar;
use crate::tabs::{Content, Message, State, Tab};
use crate::theme;
use crate::vnc_view;

pub fn view<'a>(
    state: &'a State,
    store: &'a Store,
    recent: &'a Recent,
    layout: TabLayout,
) -> Element<'a, Message> {
    if state.tabs.is_empty() {
        return container(text(fl!("tabs-empty")).style(theme::secondary_text))
            .center(Length::Fill)
            .into();
    }
    // The bar depends on the width it gets, which is only known at layout time.
    responsive(move |size| view_sized(state, store, recent, layout, size.width)).into()
}

fn view_sized<'a>(
    state: &'a State,
    store: &'a Store,
    recent: &'a Recent,
    layout: TabLayout,
    width: f32,
) -> Element<'a, Message> {
    let count = state.tabs.len();
    let new_tab = bar_button(icons::PLUS, Message::NewTabPressed);

    // Where each tab sits in the bar, as column and line, for placing its menu.
    let place_of = |index: usize| match layout {
        TabLayout::SingleLine => {
            let shown = tab_bar::single_line(count, state.active, width);
            shown.contains(&index).then(|| (index - shown.start, 0))
        }
        TabLayout::Wrap => {
            let per_line = tab_bar::per_line(width);
            Some((index % per_line, index / per_line))
        }
    };

    let (bar, hidden): (Element<'a, Message>, Vec<usize>) = match layout {
        TabLayout::SingleLine => {
            let shown = tab_bar::single_line(count, state.active, width);
            let hidden: Vec<usize> = (0..count).filter(|index| !shown.contains(index)).collect();
            let mut line = row(shown.map(|index| view_tab(state, index))).spacing(tab_bar::GAP);
            if !hidden.is_empty() {
                let more = button(text(format!("+{}", hidden.len())).size(theme::TEXT_SMALL))
                    .height(Length::Fixed(tab_bar::TAB_HEIGHT))
                    .style(theme::pill)
                    .on_press(Message::OverflowToggled);
                line = line.push(more);
            }
            (line.push(new_tab).align_y(Vertical::Center).into(), hidden)
        }
        TabLayout::Wrap => {
            let tabs = (0..count).map(|index| view_tab(state, index));
            let wrapped = row(tabs.chain(std::iter::once(new_tab)))
                .spacing(tab_bar::GAP)
                .wrap();
            let (lines, scrolls) = tab_bar::wrapped_lines(count, width);
            let bar: Element<'a, Message> = if scrolls {
                scrollable(wrapped)
                    .height(Length::Fixed(tab_bar::height(lines)))
                    .into()
            } else {
                wrapped.into()
            };
            (bar, Vec::new())
        }
    };

    let body = match state.tabs.get(state.active) {
        Some(tab) => view_content(tab, store, recent),
        None => space().into(),
    };
    let page = column![bar, body]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill);

    let mut layers = stack![page];
    if state.overflow_open && !hidden.is_empty() {
        layers = layers.push(overflow_list(state, hidden));
    }
    if let Some(index) = state.menu.and_then(|id| state.index_of(id))
        && let Some((column, line)) = place_of(index)
    {
        let position = Point::new(
            column as f32 * (tab_bar::TAB_WIDTH + tab_bar::GAP),
            (line + 1) as f32 * (tab_bar::TAB_HEIGHT + tab_bar::GAP),
        );
        layers = layers.push(tab_menu(&state.tabs[index], position));
    }
    if let Some(tab) = state.confirming_close.and_then(|id| state.index_of(id)) {
        layers = layers.push(close_question(&state.tabs[tab].title));
    }
    layers.into()
}

fn view_tab(state: &State, index: usize) -> Element<'_, Message> {
    let tab = &state.tabs[index];
    let is_active = index == state.active;
    let is_drop_target = state.hovered == Some(index)
        && state
            .dragged
            .is_some_and(|id| state.index_of(id) != Some(index));

    let shortened = tab_bar::shortened(&tab.title);
    let shown = shortened.as_deref().unwrap_or(tab.title.as_str());
    let title = text(shown.to_string())
        .wrapping(text::Wrapping::None)
        .width(Length::Fill);
    let close = button(icons::view(icons::CLOSE, theme::TEXT_SMALL))
        .padding(4)
        .style(theme::entry(false))
        .on_press(Message::CloseRequested(index));
    let content = row![title, close].spacing(4).align_y(Vertical::Center);
    let tab_box = container(content)
        .width(Length::Fixed(tab_bar::TAB_WIDTH))
        .height(Length::Fixed(tab_bar::TAB_HEIGHT))
        .padding([0, 10])
        .align_y(Vertical::Center)
        .clip(true);
    let tab_box = if is_drop_target {
        tab_box.style(theme::drop_target)
    } else {
        tab_box.style(theme::tab(is_active))
    };

    // A cut title shows in full when the pointer rests on the tab.
    let tab_box: Element<'_, Message> = match shortened {
        Some(_) => tooltip(tab_box, text(tab.title.as_str()), tooltip::Position::Bottom)
            .gap(4)
            .style(theme::floating)
            .into(),
        None => tab_box.into(),
    };
    mouse_area(tab_box)
        .on_right_press(Message::MenuRequested(index))
        // A middle click closes the tab, as in a browser.
        .on_middle_press(Message::CloseRequested(index))
        .on_press(Message::TabPressed(index))
        .on_enter(Message::TabEntered(index))
        .on_exit(Message::TabExited(index))
        .into()
}

fn bar_button<'a>(icon: icons::Icon, on_press: Message) -> Element<'a, Message> {
    let icon = container(icons::view(icon, theme::TEXT_BODY)).center_y(Length::Fill);
    button(icon)
        .height(Length::Fixed(tab_bar::TAB_HEIGHT))
        .padding([0, 10])
        .style(theme::entry(false))
        .on_press(on_press)
        .into()
}

fn view_content<'a>(tab: &'a Tab, store: &'a Store, recent: &'a Recent) -> Element<'a, Message> {
    let id = tab.id;
    match &tab.content {
        Content::Launcher(query) => launcher::view(store, recent.ids(), query)
            .map(move |message| Message::Launcher(id, message)),
        Content::Session(session) => {
            session_view::view(session).map(move |message| Message::Session(id, message))
        }
        Content::Desktop(desktop) => {
            desktop_view::view(desktop).map(move |message| Message::Desktop(id, message))
        }
        Content::Vnc(vnc) => vnc_view::view(vnc).map(move |message| Message::Vnc(id, message)),
        Content::Ftp(ftp) => ftp_view::view(ftp).map(move |message| Message::Ftp(id, message)),
    }
}

/// The tabs that do not fit on the line, as a list under the right end of the bar.
fn overflow_list(state: &State, hidden: Vec<usize>) -> Element<'_, Message> {
    let items = hidden.into_iter().map(|index| {
        button(text(state.tabs[index].title.as_str()))
            .width(Length::Fill)
            .padding([6, 10])
            .style(theme::entry(false))
            .on_press(Message::OverflowPicked(index))
            .into()
    });
    let list = container(scrollable(column(items)))
        .width(Length::Fixed(220.0))
        .max_height(320.0)
        .padding(6)
        .style(theme::floating);
    container(opaque(list))
        .align_right(Length::Fill)
        .padding(iced::Padding::default().top(tab_bar::TAB_HEIGHT + tab_bar::GAP))
        .into()
}

/// The right-click menu of a tab, under the tab, with a catcher for clicks outside it.
fn tab_menu(tab: &Tab, position: Point) -> Element<'_, Message> {
    let item = |label: String, message: Message| {
        button(text(label))
            .width(Length::Fill)
            .padding([6, 10])
            .style(theme::entry(false))
            .on_press(message)
    };
    let mut items = column![];
    if tab.unsaved.is_some() {
        items = items.push(item(fl!("tabs-menu-save"), Message::MenuSavePressed));
    }
    items = items.push(item(fl!("tabs-menu-close"), Message::MenuClosePressed));
    let panel = container(items)
        .width(Length::Fixed(220.0))
        .padding(6)
        .style(theme::floating);

    let outside = mouse_area(space().width(Length::Fill).height(Length::Fill))
        .on_press(Message::MenuDismissed)
        .on_right_press(Message::MenuDismissed);
    stack![opaque(outside), pin(opaque(panel)).position(position)].into()
}

/// Asks before a live session is cut. It covers the tab so nothing else can be clicked.
fn close_question(title: &str) -> Element<'_, Message> {
    let buttons = row![
        button(text(fl!("tabs-close-yes")))
            .style(theme::danger)
            .on_press(Message::CloseConfirmed),
        button(text(fl!("tabs-close-no")))
            .style(theme::pill)
            .on_press(Message::CloseCancelled),
    ]
    .spacing(8);
    let question = column![
        text(fl!("tabs-close-question", name = title)).size(theme::TEXT_TITLE),
        text(fl!("tabs-close-explain")).style(theme::secondary_text),
        buttons,
    ]
    .spacing(theme::GAP);
    let dialog = container(question)
        .max_width(380.0)
        .padding(theme::PANEL_PADDING)
        .style(theme::floating);
    opaque(container(dialog).center(Length::Fill))
}
