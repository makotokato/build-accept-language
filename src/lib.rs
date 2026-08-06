use icu_locale::{LanguageIdentifier, Locale, LocaleExpander, ParseError, langid};

/// Builds the language tag list for an `Accept-Language` header out of the
/// requested locales, in order of preference.
///
/// Returns the first [`ParseError`] instead if any locale is malformed. Use
/// [`generate_accept_language_lossy`] to skip the malformed ones and keep going.
pub fn generate_accept_language(locales: &[String]) -> Result<Vec<String>, ParseError> {
    let langids = locales
        .iter()
        .map(|locale| locale.parse::<Locale>().map(|locale| locale.id))
        .collect::<Result<Vec<LanguageIdentifier>, ParseError>>()?;

    Ok(build_accept_language(&langids))
}

/// Same as [`generate_accept_language`], but a locale that does not parse is
/// skipped instead of failing the whole list: one unusable entry should not cost
/// the user the languages around it.
pub fn generate_accept_language_lossy(locales: &[String]) -> Vec<String> {
    let langids = locales
        .iter()
        .filter_map(|locale| locale.parse::<Locale>().ok())
        .map(|locale| locale.id)
        .collect::<Vec<LanguageIdentifier>>();

    build_accept_language(&langids)
}

fn push_unique(tags: &mut Vec<String>, langid: &LanguageIdentifier) {
    let tag = langid.to_string();
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

fn build_accept_language(langids: &[LanguageIdentifier]) -> Vec<String> {
    let expander = LocaleExpander::new_extended();
    let mut tags = Vec::new();

    for (index, langid) in langids.iter().enumerate() {
        let bare = LanguageIdentifier::from(langid.language);

        // The region is what the user asked for, and it implies the script, so
        // `zh-Hans-CN` goes out as `zh-CN`: servers negotiate a region far more
        // reliably than a script subtag.
        let regional = langid.region.map(|region| {
            let mut regional = bare.clone();
            regional.region = Some(region);
            regional
        });
        if let Some(regional) = &regional {
            push_unique(&mut tags, regional);
        }

        // Minimizing keeps only what the language does not already imply, which
        // is how a meaningful script or variant still reaches the server
        // (`sr-Latn`, `ca-valencia`) after the region-only tag above dropped it.
        let mut minimized = langid.clone();
        expander.minimize(&mut minimized);
        if minimized != bare && Some(&minimized) != regional.as_ref() {
            push_unique(&mut tags, &minimized);
        }

        // The bare language matches worse than any region the user actually
        // asked for, so it waits until the last locale that uses it: someone
        // requesting es-MX then es-ES prefers both over plain es.
        let last_of_language = !langids[index + 1..]
            .iter()
            .any(|other| other.language == langid.language);
        if last_of_language {
            push_unique(&mut tags, &bare);
        }
    }

    push_unique(&mut tags, &langid!("en-US"));
    push_unique(&mut tags, &langid!("en"));

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_accept_language_simple() {
        let locales = vec!["en-US".to_string()];
        let expected = vec!["en-US".to_string(), "en".to_string()];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_ignore_case() {
        let locales = vec!["EN-us".to_string(), "en-us".to_string()];
        let expected = vec!["en-US".to_string(), "en".to_string()];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_multiple_regins() {
        let locales = vec![
            "es-MX".to_string(),
            "es-ES".to_string(),
            "es-AR".to_string(),
            "es-CL".to_string(),
            "en-US".to_string(),
            "en-GB".to_string(),
            "en-CA".to_string(),
        ];
        let expected = vec![
            "es-MX".to_string(),
            "es-ES".to_string(),
            "es-AR".to_string(),
            "es-CL".to_string(),
            "es".to_string(),
            "en-US".to_string(),
            "en-GB".to_string(),
            "en-CA".to_string(),
            "en".to_string(),
        ];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_duplicated() {
        let locales = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "fr-FR".to_string(),
            "en-US".to_string(),
            "fr-FR".to_string(),
        ];
        let expected = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "fr-FR".to_string(),
            "en".to_string(),
            "fr".to_string(),
        ];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_duplicated2() {
        let locales = vec![
            "en-US".to_string(),
            "zh-TW".to_string(),
            "ja-JP".to_string(),
            "zh-CN".to_string(),
        ];
        let expected = vec![
            "en-US".to_string(),
            "en".to_string(),
            "zh-TW".to_string(),
            "ja-JP".to_string(),
            "ja".to_string(),
            "zh-CN".to_string(),
            "zh".to_string(),
        ];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_non_us() {
        let locales = vec![
            "ja-JP".to_string(),
            "zh-Hans-CN".to_string(),
            "fr-FR".to_string(),
            "zh-Hant-TW".to_string(),
        ];
        let expected = vec![
            "ja-JP".to_string(),
            "ja".to_string(),
            "zh-CN".to_string(),
            "fr-FR".to_string(),
            "fr".to_string(),
            "zh-TW".to_string(),
            "zh".to_string(),
            "en-US".to_string(),
            "en".to_string(),
        ];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_unicode_extension() {
        let locales = vec![
            "en-US-u-mu-celsius".to_string(),
            "ja-JP-u-ca-japanese-hc-h23".to_string(),
        ];
        let expected = vec![
            "en-US".to_string(),
            "en".to_string(),
            "ja-JP".to_string(),
            "ja".to_string(),
        ];

        let result = generate_accept_language(&locales).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_accept_language_invalid() {
        let locales = vec!["ja-JP".to_string(), "not a locale".to_string()];

        assert!(generate_accept_language(&locales).is_err());
    }

    #[test]
    fn test_generate_accept_language_lossy_invalid() {
        let locales = vec!["not a locale".to_string()];
        let expected = vec!["en-US".to_string(), "en".to_string()];

        assert_eq!(generate_accept_language_lossy(&locales), expected);
    }

    #[test]
    fn test_generate_accept_language_lossy_invalid_among_valid() {
        let locales = vec![
            "ja-JP".to_string(),
            "en_US".to_string(),
            "not a locale".to_string(),
            "de-DE".to_string(),
        ];
        let expected = vec![
            "ja-JP".to_string(),
            "ja".to_string(),
            "de-DE".to_string(),
            "de".to_string(),
            "en-US".to_string(),
            "en".to_string(),
        ];

        assert_eq!(generate_accept_language_lossy(&locales), expected);
    }

    #[test]
    fn test_generate_accept_language_lossy_matches_strict_when_valid() {
        let locales = vec!["ja-JP".to_string(), "zh-Hans-CN".to_string()];

        assert_eq!(
            generate_accept_language_lossy(&locales),
            generate_accept_language(&locales).unwrap()
        );
    }
}
