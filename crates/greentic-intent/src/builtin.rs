//! Built-in compiled-in locale bundles + gazetteer. Each is gated by its
//! own feature so consumers can keep footprint minimal.
//!
//! Phase A: en-GB locale + a small set of well-known cities and countries.
//! The other locales described in the design doc (en-US, fr-FR, es-ES,
//! nl-NL, de-DE) slot in here as parallel functions and are registered
//! alongside en-GB by `with_builtin_locales()`.

/// Build the en-GB locale bundle.
#[cfg(feature = "builtin-locales")]
pub fn en_gb_bundle() -> crate::locale::LocaleBundle {
    use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};
    use std::collections::HashMap;

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

/// Populate `g` with the tiny built-in city + country set. Inserts via
/// `Gazetteer::insert` so it stacks with any caller-supplied entries.
#[cfg(feature = "builtin-gazetteer")]
pub fn populate_gazetteer(g: &mut crate::gazetteer::Gazetteer) {
    use crate::gazetteer::GazetteerRecord;

    // Cities. Aliases are conservative: only forms that won't collide
    // with common English words (e.g. no bare "us" for United States,
    // no bare "la" for Los Angeles).
    let cities: &[(&str, &str, &[&str])] = &[
        ("London", "GB", &["londres", "londen"]),
        ("Paris", "FR", &["parís"]),
        ("New York", "US", &["nyc", "new york city"]),
        ("Tokyo", "JP", &["tōkyō"]),
        ("Berlin", "DE", &[]),
        ("Amsterdam", "NL", &[]),
        ("Madrid", "ES", &[]),
        ("Brussels", "BE", &["bruxelles", "brussel"]),
        ("Rome", "IT", &["roma"]),
        ("Lisbon", "PT", &["lisboa"]),
        ("Dublin", "IE", &[]),
        ("Edinburgh", "GB", &[]),
        ("Manchester", "GB", &[]),
        ("Cape Town", "ZA", &[]),
        ("San Francisco", "US", &["san fran"]),
    ];
    for (name, country, aliases) in cities {
        g.insert(
            *name,
            GazetteerRecord {
                canonical: (*name).into(),
                country: Some((*country).into()),
                kind_hint: Some("city".into()),
                aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
            },
        );
    }

    let countries: &[(&str, &str, &[&str])] = &[
        ("United Kingdom", "GB", &["uk", "great britain"]),
        ("United States", "US", &["usa"]),
        ("France", "FR", &[]),
        ("Germany", "DE", &["deutschland"]),
        ("Spain", "ES", &["españa"]),
        ("Netherlands", "NL", &["nederland", "holland"]),
        ("Italy", "IT", &["italia"]),
        ("Portugal", "PT", &[]),
        ("Belgium", "BE", &["belgique", "belgië"]),
        ("Ireland", "IE", &[]),
        ("Japan", "JP", &[]),
    ];
    for (name, code, aliases) in countries {
        g.insert(
            *name,
            GazetteerRecord {
                canonical: (*name).into(),
                country: Some((*code).into()),
                kind_hint: Some("country".into()),
                aliases: aliases.iter().map(|s| (*s).to_string()).collect(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "builtin-gazetteer")]
    #[test]
    fn builtin_gazetteer_populates_expected_canonical_forms() {
        let mut g = crate::gazetteer::Gazetteer::default();
        super::populate_gazetteer(&mut g);
        assert_eq!(g.lookup("london").unwrap().canonical, "London");
        assert_eq!(g.lookup("Londres").unwrap().canonical, "London");
        assert_eq!(g.lookup("NYC").unwrap().canonical, "New York");
        assert_eq!(g.lookup("new york").unwrap().canonical, "New York");
        assert_eq!(g.lookup("uk").unwrap().canonical, "United Kingdom");
    }

    #[cfg(feature = "builtin-gazetteer")]
    #[test]
    fn builtin_gazetteer_max_span_supports_multi_word_entries() {
        let mut g = crate::gazetteer::Gazetteer::default();
        super::populate_gazetteer(&mut g);
        // "new york city" / "great britain" are 3 words; "united kingdom" is 2.
        assert!(g.max_span_words() >= 3);
    }
}
