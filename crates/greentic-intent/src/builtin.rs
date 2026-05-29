//! Built-in compiled-in locale bundles. Only available with the
//! `builtin-locales` feature.
//!
//! Phase A: en-GB only. The other locales described in the design doc
//! (en-US, fr-FR, es-ES, nl-NL, de-DE) slot in here as parallel functions
//! and are registered alongside en-GB by `with_builtin_locales()`.

use std::collections::HashMap;

use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};

/// Build the en-GB locale bundle.
pub fn en_gb_bundle() -> LocaleBundle {
    LocaleBundle {
        locale: "en-GB".into(),
        relative_days: HashMap::from([
            ("today".into(), 0),
            ("tomorrow".into(), 1),
            ("yesterday".into(), -1),
        ]),
        weekdays: HashMap::from([
            ("monday".into(), 1),
            ("mon".into(), 1),
            ("tuesday".into(), 2),
            ("tue".into(), 2),
            ("tues".into(), 2),
            ("wednesday".into(), 3),
            ("wed".into(), 3),
            ("thursday".into(), 4),
            ("thu".into(), 4),
            ("thurs".into(), 4),
            ("friday".into(), 5),
            ("fri".into(), 5),
            ("saturday".into(), 6),
            ("sat".into(), 6),
            ("sunday".into(), 7),
            ("sun".into(), 7),
        ]),
        modifiers: HashMap::from([
            ("next".into(), RelativeModifier::Next),
            ("this".into(), RelativeModifier::This),
            ("last".into(), RelativeModifier::Last),
            ("in".into(), RelativeModifier::In),
            ("ago".into(), RelativeModifier::Ago),
        ]),
        location_prepositions: HashMap::from([
            ("in".into(), LocationRole::In),
            ("at".into(), LocationRole::In),
            ("from".into(), LocationRole::From),
            ("to".into(), LocationRole::To),
            ("near".into(), LocationRole::Near),
            ("around".into(), LocationRole::Around),
        ]),
    }
}
