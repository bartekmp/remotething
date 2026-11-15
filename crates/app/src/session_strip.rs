//! The strip under an SSH terminal and the details behind its (i) button.

use crate::i18n::fl;
use crate::jump;
use crate::meter;
use crate::session::{Message, Method, State};
use crate::session_update::Live;
use crate::status_bar::Bar;

pub(crate) fn bar(state: &State, live: &Live) -> Bar<Message> {
    let connection = &state.connection;
    let summary = if state.username.is_empty() {
        format!("{}:{}", connection.host, connection.port)
    } else {
        format!("{}@{}:{}", state.username, connection.host, connection.port)
    };
    let rate = match &live.meter.reading {
        Some(reading) => format!(
            "↓ {}/s  ↑ {}/s",
            meter::bytes(reading.received_per_second),
            meter::bytes(reading.sent_per_second)
        ),
        None => String::new(),
    };
    Bar {
        summary,
        live: rate,
        details: details(state, live),
        details_open: live.details_open,
        actions: Vec::new(),
    }
}

fn method_label(method: &Method) -> String {
    match method {
        Method::Agent => fl!("details-method-agent"),
        Method::Interactive => fl!("details-method-interactive"),
        Method::Password(_) => fl!("details-method-password"),
        Method::KeyFile { .. } => fl!("details-method-key"),
    }
}

/// How the connection reaches the server: its address, or the jump hosts it goes through.
fn route(state: &State, live: &Live) -> String {
    let hops: Vec<String> = state
        .jumps
        .iter()
        .filter_map(jump::Ready::hop)
        .map(|hop| format!("{}:{}", hop.endpoint.host, hop.endpoint.port))
        .collect();
    match (live.details.peer, hops.is_empty()) {
        (Some(peer), true) => peer.to_string(),
        _ => fl!("details-via", hosts = hops.join(" → ")),
    }
}

fn details(state: &State, live: &Live) -> Vec<(String, String)> {
    let connection = &state.connection;
    let mut lines = vec![
        (fl!("details-protocol"), "SSH".to_string()),
        (fl!("details-host"), connection.host.clone()),
        (fl!("details-address"), route(state, live)),
        (fl!("details-user"), state.username.clone()),
        (fl!("details-method"), method_label(&state.method)),
        (fl!("details-server"), live.details.server.clone()),
        (
            fl!("details-host-key"),
            live.details.host_key_algorithm.clone(),
        ),
        (fl!("details-fingerprint"), live.details.fingerprint.clone()),
        (
            fl!("details-terminal"),
            format!("{} × {}", live.screen.columns(), live.screen.lines()),
        ),
    ];
    let forwards = live.forwards.open_forwards();
    if !forwards.is_empty() {
        lines.push((fl!("details-forwards"), forwards.join(", ")));
    }
    if let Some(reading) = &live.meter.reading {
        lines.push((
            fl!("details-received"),
            format!(
                "{} ({}/s)",
                meter::bytes(reading.totals.received),
                meter::bytes(reading.received_per_second)
            ),
        ));
        lines.push((
            fl!("details-sent"),
            format!(
                "{} ({}/s)",
                meter::bytes(reading.totals.sent),
                meter::bytes(reading.sent_per_second)
            ),
        ));
        lines.push((fl!("details-uptime"), meter::clock(reading.uptime)));
    }
    lines
}
