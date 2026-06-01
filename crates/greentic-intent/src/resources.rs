//! Shared, preloaded resources handed to extractors at request time.
//!
//! Phase A: empty placeholder. Gazetteer + per-locale dictionaries fill
//! in incrementally and ride along here so extractors don't reach into
//! global state.

use crate::gazetteer::Gazetteer;
use crate::locale::LocaleBundle;

/// Bag of preloaded resources passed by reference into each extractor.
#[derive(Debug, Default)]
pub struct IntentResources {
    /// Gazetteer (locations, organisations, …).
    pub gazetteer: Gazetteer,
    /// Locale bundles keyed by BCP-47 tag.
    pub locales: std::collections::HashMap<String, LocaleBundle>,
}
