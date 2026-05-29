//! Built-in compiled-in locale bundles + gazetteer. Each is gated by its
//! own feature so consumers can keep footprint minimal.
//!
//! Phase A: en-GB / en-US / fr-FR / es-ES / nl-NL / de-DE locales + a
//! small set of well-known cities and countries. All locales are
//! registered in one shot by `with_builtin_locales()`. Vocabularies for
//! en-GB / fr-FR / es-ES / nl-NL come from the design doc verbatim;
//! en-US clones en-GB (same vocabulary, different tag) and de-DE adds a
//! conservative German subset.

/// All built-in locale bundles in one shot. Used by
/// `IntentEngineBuilder::with_builtin_locales`.
#[cfg(feature = "builtin-locales")]
pub fn all_builtin_locales() -> Vec<crate::locale::LocaleBundle> {
    vec![
        en_gb_bundle(),
        en_us_bundle(),
        fr_fr_bundle(),
        es_es_bundle(),
        nl_nl_bundle(),
        de_de_bundle(),
    ]
}

/// en-GB locale bundle.
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
        time_words: HashMap::from([
            ("noon".into(), (12u8, 0u8)),
            ("midday".into(), (12u8, 0u8)),
            ("midnight".into(), (0u8, 0u8)),
        ]),
    }
}

/// en-US locale bundle. Same vocabulary as en-GB with a different tag.
#[cfg(feature = "builtin-locales")]
pub fn en_us_bundle() -> crate::locale::LocaleBundle {
    let mut b = en_gb_bundle();
    b.locale = "en-US".into();
    b
}

/// fr-FR locale bundle.
///
/// Note: `à` is mapped to `In` per the design doc table. In real French
/// it ambiguously serves both "at/in <city>" and "to <city>"; a Phase B
/// refinement would resolve that contextually (e.g. promote to `To` when
/// a preceding `from`-tagged location exists).
#[cfg(feature = "builtin-locales")]
pub fn fr_fr_bundle() -> crate::locale::LocaleBundle {
    use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};
    use std::collections::HashMap;

    LocaleBundle {
        locale: "fr-FR".into(),
        relative_days: HashMap::from([
            ("aujourd'hui".into(), 0),
            ("aujourdhui".into(), 0),
            ("demain".into(), 1),
            ("hier".into(), -1),
        ]),
        weekdays: HashMap::from([
            ("lundi".into(), 1),
            ("mardi".into(), 2),
            ("mercredi".into(), 3),
            ("jeudi".into(), 4),
            ("vendredi".into(), 5),
            ("samedi".into(), 6),
            ("dimanche".into(), 7),
        ]),
        modifiers: HashMap::from([
            ("prochain".into(), RelativeModifier::Next),
            ("prochaine".into(), RelativeModifier::Next),
            ("ce".into(), RelativeModifier::This),
            ("cette".into(), RelativeModifier::This),
            ("dernier".into(), RelativeModifier::Last),
            ("dernière".into(), RelativeModifier::Last),
            ("derniere".into(), RelativeModifier::Last),
            ("dans".into(), RelativeModifier::In),
        ]),
        location_prepositions: HashMap::from([
            ("à".into(), LocationRole::In),
            ("a".into(), LocationRole::In),
            ("en".into(), LocationRole::In),
            ("dans".into(), LocationRole::In),
            ("de".into(), LocationRole::From),
            ("depuis".into(), LocationRole::From),
            ("vers".into(), LocationRole::To),
        ]),
        time_words: HashMap::from([("midi".into(), (12u8, 0u8)), ("minuit".into(), (0u8, 0u8))]),
    }
}

/// es-ES locale bundle.
#[cfg(feature = "builtin-locales")]
pub fn es_es_bundle() -> crate::locale::LocaleBundle {
    use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};
    use std::collections::HashMap;

    LocaleBundle {
        locale: "es-ES".into(),
        relative_days: HashMap::from([
            ("hoy".into(), 0),
            ("mañana".into(), 1),
            ("manana".into(), 1),
            ("ayer".into(), -1),
        ]),
        weekdays: HashMap::from([
            ("lunes".into(), 1),
            ("martes".into(), 2),
            ("miércoles".into(), 3),
            ("miercoles".into(), 3),
            ("jueves".into(), 4),
            ("viernes".into(), 5),
            ("sábado".into(), 6),
            ("sabado".into(), 6),
            ("domingo".into(), 7),
        ]),
        modifiers: HashMap::from([
            ("próximo".into(), RelativeModifier::Next),
            ("proximo".into(), RelativeModifier::Next),
            ("próxima".into(), RelativeModifier::Next),
            ("proxima".into(), RelativeModifier::Next),
            ("esta".into(), RelativeModifier::This),
            ("este".into(), RelativeModifier::This),
            ("pasado".into(), RelativeModifier::Last),
            ("pasada".into(), RelativeModifier::Last),
            ("en".into(), RelativeModifier::In),
            ("hace".into(), RelativeModifier::Ago),
        ]),
        location_prepositions: HashMap::from([
            ("en".into(), LocationRole::In),
            ("desde".into(), LocationRole::From),
            ("de".into(), LocationRole::From),
            ("a".into(), LocationRole::To),
            ("hacia".into(), LocationRole::To),
            ("cerca".into(), LocationRole::Near),
        ]),
        time_words: HashMap::from([
            ("mediodía".into(), (12u8, 0u8)),
            ("mediodia".into(), (12u8, 0u8)),
            ("medianoche".into(), (0u8, 0u8)),
        ]),
    }
}

/// nl-NL locale bundle.
#[cfg(feature = "builtin-locales")]
pub fn nl_nl_bundle() -> crate::locale::LocaleBundle {
    use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};
    use std::collections::HashMap;

    LocaleBundle {
        locale: "nl-NL".into(),
        relative_days: HashMap::from([
            ("vandaag".into(), 0),
            ("morgen".into(), 1),
            ("gisteren".into(), -1),
        ]),
        weekdays: HashMap::from([
            ("maandag".into(), 1),
            ("ma".into(), 1),
            ("dinsdag".into(), 2),
            ("di".into(), 2),
            ("woensdag".into(), 3),
            ("wo".into(), 3),
            ("donderdag".into(), 4),
            ("do".into(), 4),
            ("vrijdag".into(), 5),
            ("vr".into(), 5),
            ("zaterdag".into(), 6),
            ("za".into(), 6),
            ("zondag".into(), 7),
            ("zo".into(), 7),
        ]),
        modifiers: HashMap::from([
            ("volgende".into(), RelativeModifier::Next),
            ("deze".into(), RelativeModifier::This),
            ("vorige".into(), RelativeModifier::Last),
            ("over".into(), RelativeModifier::In),
            ("geleden".into(), RelativeModifier::Ago),
        ]),
        location_prepositions: HashMap::from([
            ("in".into(), LocationRole::In),
            ("te".into(), LocationRole::In),
            ("naar".into(), LocationRole::To),
            ("van".into(), LocationRole::From),
            ("vanuit".into(), LocationRole::From),
            ("bij".into(), LocationRole::Near),
        ]),
        time_words: HashMap::from([("middernacht".into(), (0u8, 0u8))]),
    }
}

/// de-DE locale bundle. Conservative subset — German prepositions are
/// case-driven and ambiguous (`bei` covers near/at, `in` covers in/to
/// depending on case), so we map the unambiguous core only.
#[cfg(feature = "builtin-locales")]
pub fn de_de_bundle() -> crate::locale::LocaleBundle {
    use crate::locale::{LocaleBundle, LocationRole, RelativeModifier};
    use std::collections::HashMap;

    LocaleBundle {
        locale: "de-DE".into(),
        relative_days: HashMap::from([
            ("heute".into(), 0),
            ("morgen".into(), 1),
            ("übermorgen".into(), 2),
            ("uebermorgen".into(), 2),
            ("gestern".into(), -1),
            ("vorgestern".into(), -2),
        ]),
        weekdays: HashMap::from([
            ("montag".into(), 1),
            ("mo".into(), 1),
            ("dienstag".into(), 2),
            ("di".into(), 2),
            ("mittwoch".into(), 3),
            ("mi".into(), 3),
            ("donnerstag".into(), 4),
            ("do".into(), 4),
            ("freitag".into(), 5),
            ("fr".into(), 5),
            ("samstag".into(), 6),
            ("sa".into(), 6),
            ("sonntag".into(), 7),
            ("so".into(), 7),
        ]),
        modifiers: HashMap::from([
            ("nächste".into(), RelativeModifier::Next),
            ("nächsten".into(), RelativeModifier::Next),
            ("nächster".into(), RelativeModifier::Next),
            ("naechste".into(), RelativeModifier::Next),
            ("naechsten".into(), RelativeModifier::Next),
            ("diese".into(), RelativeModifier::This),
            ("diesen".into(), RelativeModifier::This),
            ("dieser".into(), RelativeModifier::This),
            ("letzte".into(), RelativeModifier::Last),
            ("letzten".into(), RelativeModifier::Last),
            ("letzter".into(), RelativeModifier::Last),
            ("in".into(), RelativeModifier::In),
            ("vor".into(), RelativeModifier::Ago),
        ]),
        location_prepositions: HashMap::from([
            ("in".into(), LocationRole::In),
            ("aus".into(), LocationRole::From),
            ("von".into(), LocationRole::From),
            ("nach".into(), LocationRole::To),
            ("nahe".into(), LocationRole::Near),
            ("bei".into(), LocationRole::Near),
        ]),
        time_words: HashMap::from([
            ("mittag".into(), (12u8, 0u8)),
            ("mitternacht".into(), (0u8, 0u8)),
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
        ("Paris", "FR", &["parís", "parijs"]),
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
        ("Munich", "DE", &["münchen", "muenchen"]),
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
    #[cfg(feature = "builtin-locales")]
    #[test]
    fn all_builtin_locales_unique_tags() {
        let bundles = super::all_builtin_locales();
        let mut tags: Vec<&str> = bundles.iter().map(|b| b.locale.as_str()).collect();
        tags.sort();
        let dedup_len = {
            let mut v = tags.clone();
            v.dedup();
            v.len()
        };
        assert_eq!(tags.len(), dedup_len, "duplicate locale tags: {tags:?}");
        assert!(tags.contains(&"en-GB"));
        assert!(tags.contains(&"fr-FR"));
        assert!(tags.contains(&"es-ES"));
        assert!(tags.contains(&"nl-NL"));
        assert!(tags.contains(&"de-DE"));
        assert!(tags.contains(&"en-US"));
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn fr_fr_includes_demain_and_de() {
        let b = super::fr_fr_bundle();
        assert_eq!(b.relative_days.get("demain").copied(), Some(1));
        assert!(b.location_prepositions.contains_key("de"));
        assert!(b.location_prepositions.contains_key("à"));
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn es_es_distinguishes_in_from_to() {
        let b = super::es_es_bundle();
        use crate::locale::LocationRole;
        assert_eq!(
            b.location_prepositions.get("en").copied(),
            Some(LocationRole::In)
        );
        assert_eq!(
            b.location_prepositions.get("a").copied(),
            Some(LocationRole::To)
        );
        assert_eq!(
            b.location_prepositions.get("de").copied(),
            Some(LocationRole::From)
        );
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn nl_nl_includes_morgen_and_naar() {
        let b = super::nl_nl_bundle();
        use crate::locale::LocationRole;
        assert_eq!(b.relative_days.get("morgen").copied(), Some(1));
        assert_eq!(
            b.location_prepositions.get("naar").copied(),
            Some(LocationRole::To)
        );
        assert_eq!(
            b.location_prepositions.get("van").copied(),
            Some(LocationRole::From)
        );
    }

    #[cfg(feature = "builtin-locales")]
    #[test]
    fn de_de_includes_morgen() {
        let b = super::de_de_bundle();
        assert_eq!(b.relative_days.get("morgen").copied(), Some(1));
    }

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
        assert_eq!(g.lookup("Parijs").unwrap().canonical, "Paris");
        assert_eq!(g.lookup("München").unwrap().canonical, "Munich");
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
