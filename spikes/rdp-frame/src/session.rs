//! A blocking IronRDP session that hands decoded frames to a callback.
//!
//! Adapted from IronRDP's `screenshot` example. The real app would use the
//! async `ironrdp-tokio` flavour and dirty rectangles instead of full frames.

use std::io::ErrorKind;
use std::net::{IpAddr, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use ironrdp_connector::sspi::generator::NetworkRequest;
use ironrdp_connector::sspi::network_client::NetworkClient;
use ironrdp_connector::{self as connector, ClientConnector, ConnectionResult, Credentials};
use ironrdp_graphics::image_processing::PixelFormat;
use ironrdp_pdu::gcc::{ConnectionType, KeyboardType};
use ironrdp_pdu::rdp::capability_sets::{MajorPlatformType, RailSupportLevel};
use ironrdp_pdu::rdp::client_info::{PerformanceFlags, TimezoneInfo};
use ironrdp_session::image::DecodedImage;
use ironrdp_session::{ActiveStageBuilder, ActiveStageOutput};

use crate::tls;

const WIDTH: u16 = 1280;
const HEIGHT: u16 = 800;
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Short, so a pending frame goes out while the server is quiet.
const IDLE_READ_TIMEOUT: Duration = Duration::from_millis(50);

pub struct Target {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Network Level Authentication (CredSSP). Windows wants it, xrdp does not support it.
    pub nla: bool,
    /// Allow a non-loopback server even though its certificate is not checked.
    pub insecure: bool,
}

/// A full copy of the remote desktop, 4 bytes per pixel.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Connects and calls `on_frame` after screen updates until it returns `false`
/// or the server disconnects.
pub fn run(target: Target, mut on_frame: impl FnMut(Frame) -> bool) -> Result<()> {
    let (connection, mut framed) = connect(&target).context("connect")?;

    let mut image = DecodedImage::new(
        PixelFormat::RgbA32,
        connection.desktop_size.width,
        connection.desktop_size.height,
    );
    let mut stage = ActiveStageBuilder {
        static_channels: connection.static_channels,
        user_channel_id: connection.user_channel_id,
        io_channel_id: connection.io_channel_id,
        message_channel_id: connection.message_channel_id,
        share_id: connection.share_id,
        compression_type: connection.compression_type,
        enable_server_pointer: connection.enable_server_pointer,
        pointer_software_rendering: connection.pointer_software_rendering,
    }
    .build();

    let mut dirty = false;
    let mut last_sent = Instant::now();

    loop {
        match framed.read_pdu() {
            Ok((action, payload)) => {
                let received_at = framed.last_read_at();
                for output in stage.process(&mut image, action, &payload, received_at)? {
                    match output {
                        ActiveStageOutput::ResponseFrame(response) => {
                            framed.write_all(&response).context("write response")?;
                        }
                        ActiveStageOutput::GraphicsUpdate(_) => dirty = true,
                        ActiveStageOutput::Terminate(_) => return Ok(()),
                        _ => {}
                    }
                }
            }
            // The read timeout lets a pending frame go out while the server is quiet.
            Err(error) if is_timeout(&error) => {}
            Err(error) => return Err(error).context("read frame"),
        }

        if dirty && last_sent.elapsed() >= FRAME_INTERVAL {
            if !on_frame(copy_frame(&image)) {
                return Ok(());
            }
            dirty = false;
            last_sent = Instant::now();
        }
    }
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
}

fn copy_frame(image: &DecodedImage) -> Frame {
    let mut rgba = image.data().to_vec();
    // The decoder leaves alpha undefined; the UI needs opaque pixels.
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    Frame {
        width: u32::from(image.width()),
        height: u32::from(image.height()),
        rgba,
    }
}

type TlsFramed = ironrdp_blocking::Framed<tls::Stream>;

fn connect(target: &Target) -> Result<(ConnectionResult, TlsFramed)> {
    let tcp = TcpStream::connect((target.host.as_str(), target.port)).context("TCP connect")?;
    check_unverified_peer(tcp.peer_addr()?.ip(), target.insecure)?;
    tcp.set_read_timeout(Some(CONNECT_TIMEOUT))?;
    // A second handle to the same socket, to shorten the timeout once connected.
    let socket = tcp.try_clone()?;
    let client_addr = tcp.local_addr()?;

    let mut framed = ironrdp_blocking::Framed::new(tcp);
    let mut connector = ClientConnector::new(config(target), client_addr);

    let should_upgrade =
        ironrdp_blocking::connect_begin(&mut framed, &mut connector).context("begin connection")?;
    let tcp = framed.into_inner_no_leftover();
    let (stream, server_public_key) = tls::upgrade(tcp, &target.host).context("TLS upgrade")?;
    let upgraded = ironrdp_blocking::mark_as_upgraded(should_upgrade, &mut connector);

    let mut framed = ironrdp_blocking::Framed::new(stream);
    let connection = ironrdp_blocking::connect_finalize(
        upgraded,
        connector,
        &mut framed,
        &mut NoKerberos,
        target.host.clone().into(),
        server_public_key,
        None,
    )
    .context("finalize connection")?
    // The spike does not follow server redirections.
    .into_connected()?;

    socket.set_read_timeout(Some(IDLE_READ_TIMEOUT))?;
    Ok((connection, framed))
}

/// The spike does not verify who it talks to, so it only leaves the machine on request.
fn check_unverified_peer(peer: IpAddr, insecure: bool) -> Result<()> {
    if peer.is_loopback() || insecure {
        return Ok(());
    }
    bail!(
        "{peer} is not loopback and its identity is not verified; pass --insecure to connect anyway"
    )
}

/// CredSSP only needs the network for Kerberos. NTLM, which the spike uses, does not.
struct NoKerberos;

impl NetworkClient for NoKerberos {
    fn send(&self, _request: &NetworkRequest) -> ironrdp_connector::sspi::Result<Vec<u8>> {
        Err(ironrdp_connector::sspi::Error::new(
            ironrdp_connector::sspi::ErrorKind::NoAuthenticatingAuthority,
            "Kerberos is not wired up in this spike",
        ))
    }
}

fn config(target: &Target) -> connector::Config {
    connector::Config {
        credentials: Credentials::UsernamePassword {
            username: target.username.clone(),
            password: target.password.clone(),
        },
        domain: None,
        enable_standard_rdp_security: false,
        enable_tls: !target.nla,
        enable_credssp: target.nla,
        keyboard_type: KeyboardType::IBM_ENHANCED,
        keyboard_subtype: 0,
        keyboard_layout: 0,
        keyboard_functional_keys_count: 12,
        connection_type: ConnectionType::Lan,
        ime_file_name: String::new(),
        dig_product_id: String::new(),
        desktop_size: connector::DesktopSize {
            width: WIDTH,
            height: HEIGHT,
        },
        bitmap: None,
        monitor_layout: None,
        desktop_physical_size: None,
        desktop_orientation: None,
        device_scale_factor: None,
        monitor_extended_layout: None,
        relative_mouse_input: false,
        remote_application_mode: false,
        rail_support_level: RailSupportLevel::empty(),
        client_build: 0,
        client_name: "remotething-spike".to_owned(),
        client_dir: "C:\\Windows\\System32\\mstscax.dll".to_owned(),
        #[cfg(windows)]
        platform: MajorPlatformType::WINDOWS,
        #[cfg(not(windows))]
        platform: MajorPlatformType::UNIX,
        enable_server_pointer: false,
        request_data: None,
        autologon: true,
        enable_audio_playback: false,
        enable_audio_capture: false,
        support_dyn_vc_gfx_protocol: false,
        compression_type: None,
        pointer_software_rendering: true,
        multitransport_flags: None,
        performance_flags: PerformanceFlags::default(),
        desktop_scale_factor: 0,
        hardware_id: None,
        license_cache: None,
        timezone_info: TimezoneInfo::default(),
        alternate_shell: String::new(),
        work_dir: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_is_allowed_without_the_flag() {
        assert!(check_unverified_peer("127.0.0.1".parse().unwrap(), false).is_ok());
    }

    #[test]
    fn a_remote_host_is_refused_without_the_flag() {
        assert!(check_unverified_peer("192.0.2.10".parse().unwrap(), false).is_err());
    }

    #[test]
    fn a_remote_host_is_allowed_with_the_flag() {
        assert!(check_unverified_peer("192.0.2.10".parse().unwrap(), true).is_ok());
    }
}
