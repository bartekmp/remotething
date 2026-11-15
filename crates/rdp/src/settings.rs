//! The settings IronRDP's connector is started with.

use ironrdp_connector::{self as connector, Credentials};
use ironrdp_pdu::gcc::KeyboardType;
use ironrdp_pdu::rdp::capability_sets::{MajorPlatformType, RailSupportLevel};
use ironrdp_pdu::rdp::client_info::TimezoneInfo;

use crate::quality;
use crate::session::Target;

/// What the picture is sent as: RemoteFX when the server takes it, otherwise classic bitmaps.
pub(crate) const CODEC: &str = "RemoteFX, or bitmaps (RLE)";
pub(crate) const SECURITY: &str = "TLS with NLA (CredSSP)";

/// Connector settings for `target`. The connection always uses NLA (CredSSP).
pub(crate) fn connector_config(target: &Target) -> connector::Config {
    let profile = target.quality.profile();
    connector::Config {
        credentials: Credentials::UsernamePassword {
            username: target.username.clone(),
            password: target.password.expose().to_string(),
        },
        domain: target.domain.clone(),
        enable_standard_rdp_security: false,
        enable_tls: false,
        enable_credssp: true,
        keyboard_type: KeyboardType::IBM_ENHANCED,
        keyboard_subtype: 0,
        keyboard_layout: 0,
        keyboard_functional_keys_count: 12,
        connection_type: profile.connection_type,
        ime_file_name: String::new(),
        dig_product_id: String::new(),
        desktop_size: connector::DesktopSize {
            width: target.width,
            height: target.height,
        },
        bitmap: quality::bitmap_config(profile.color_depth),
        monitor_layout: None,
        desktop_physical_size: None,
        desktop_orientation: None,
        device_scale_factor: None,
        monitor_extended_layout: None,
        relative_mouse_input: false,
        remote_application_mode: false,
        rail_support_level: RailSupportLevel::empty(),
        client_build: 0,
        client_name: "remotething".to_owned(),
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
        performance_flags: profile.performance,
        desktop_scale_factor: 0,
        hardware_id: None,
        license_cache: None,
        timezone_info: TimezoneInfo::default(),
        alternate_shell: String::new(),
        work_dir: String::new(),
    }
}
