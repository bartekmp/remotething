//! Translations. No string literal reaches a widget; UI code calls `fl!`.

use std::sync::{LazyLock, OnceLock};

use i18n_embed::fluent::{FluentLanguageLoader, fluent_language_loader};
use i18n_embed::unic_langid::LanguageIdentifier;
use i18n_embed::{DesktopLanguageRequester, LanguageLoader as _};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "locales"]
struct Catalogues;

/// The language chosen in Settings. Unset, or `None`, follows the system.
static LANGUAGE_OVERRIDE: OnceLock<Option<String>> = OnceLock::new();

/// Fixes the language for this run. Call before the first `fl!`; later calls do nothing.
pub fn set_language_override(language: Option<String>) {
    let _ = LANGUAGE_OVERRIDE.set(language);
}

/// The loaded catalogues: the chosen or system language when we ship it, English otherwise.
pub static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader = fluent_language_loader!();
    loader
        .load_fallback_language(&Catalogues)
        .expect("the English catalogue is embedded in the binary");

    // A missing translation is not an error: English stays loaded.
    let _ = i18n_embed::select(&loader, &Catalogues, &requested_languages());

    loader
});

fn requested_languages() -> Vec<LanguageIdentifier> {
    let chosen = LANGUAGE_OVERRIDE.get().cloned().flatten();
    if let Some(language) = chosen.and_then(|code| code.parse().ok()) {
        return vec![language];
    }
    DesktopLanguageRequester::requested_languages()
}

/// The codes of every language embedded in the binary, sorted.
pub fn available_languages() -> Vec<String> {
    let mut codes: Vec<String> = LOADER
        .available_languages(&Catalogues)
        .unwrap_or_default()
        .iter()
        .map(|language| language.to_string())
        .collect();
    codes.sort();
    codes
}

/// Looks up a message by key. An unknown key fails the build.
macro_rules! fl {
    ($key:literal) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $key)
    };
    ($key:literal, $($arguments:tt)*) => {
        i18n_embed_fl::fl!($crate::i18n::LOADER, $key, $($arguments)*)
    };
}
pub(crate) use fl;
