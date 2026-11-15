//! The server changes the desktop size by deactivating the session and
//! running the capability exchange again. This answers it.

use std::io::ErrorKind;

use ironrdp_connector::connection_activation::{
    ConnectionActivationSequence, ConnectionActivationState,
};
use ironrdp_connector::{Config, Sequence};
use ironrdp_core::WriteBuf;
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_session::ActiveStage;
use ironrdp_session::image::DecodedImage;

use crate::session::TlsFramed;

type BoxedError = Box<dyn std::error::Error>;

/// Runs the exchange to its end and returns the empty picture of the new size.
pub(crate) fn run(
    framed: &mut TlsFramed,
    stage: &mut ActiveStage,
    config: &Config,
    channels: (u16, u16),
) -> Result<DecodedImage, BoxedError> {
    let (io_channel_id, user_channel_id) = channels;
    let mut sequence =
        ConnectionActivationSequence::new(config.clone(), io_channel_id, user_channel_id);
    let mut buffer = WriteBuf::new();
    loop {
        step(framed, &mut sequence, &mut buffer)?;
        let ConnectionActivationState::Finalized {
            desktop_size,
            share_id,
            static_channel_chunk_size,
            enable_server_pointer,
            pointer_software_rendering,
            ..
        } = sequence.connection_activation_state()
        else {
            continue;
        };
        let accepted = stage.reactivate(
            io_channel_id,
            user_channel_id,
            share_id,
            enable_server_pointer,
            pointer_software_rendering,
            static_channel_chunk_size,
        );
        if !accepted {
            return Err("the server sent an invalid channel chunk size".into());
        }
        return Ok(DecodedImage::new(
            PixelFormat::RgbA32,
            desktop_size.width,
            desktop_size.height,
        ));
    }
}

/// Reads what the sequence waits for, if anything, and sends its answer.
fn step(
    framed: &mut TlsFramed,
    sequence: &mut ConnectionActivationSequence,
    buffer: &mut WriteBuf,
) -> Result<(), BoxedError> {
    buffer.clear();
    let written = match sequence.next_pdu_hint() {
        Some(hint) => {
            let pdu = loop {
                match framed.read_by_hint(hint) {
                    Ok(pdu) => break pdu,
                    // The socket has a short timeout for the main loop; here we just wait.
                    Err(error) if is_timeout(&error) => {}
                    Err(error) => return Err(error.into()),
                }
            };
            sequence.step(&pdu, framed.last_read_at(), buffer)?
        }
        None => sequence.step_no_input(buffer)?,
    };
    if let Some(length) = written.size() {
        framed.write_all(&buffer[..length])?;
    }
    Ok(())
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}
