//! A callout's tone (E21, ADR 0069): which of the five colour schemes it is drawn in.
//!
//! The tone is stored by name (`info`, `success`, `warning`, `error`, `note`). A name this version
//! doesn't know, from a newer version, is kept as it was and reads as [`CalloutTone::Other`], so saving
//! the page never loses it (rule 5); the UI draws it like a note.

/// How a callout looks, and what it signals.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CalloutTone {
    /// Blue: context and FYIs. The tone a new callout starts with.
    Info,
    /// Green: something done or confirmed.
    Success,
    /// Amber: be careful.
    Warning,
    /// Red: a problem that blocks.
    Error,
    /// Grey: a tip or an aside.
    Note,
    /// A tone from a newer version, kept as stored.
    Other(String),
}

impl CalloutTone {
    /// The five tones this version knows, in the order pickers show them.
    pub const ALL: [Self; 5] = [Self::Info, Self::Success, Self::Warning, Self::Error, Self::Note];

    /// The stored name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Note => "note",
            Self::Other(name) => name,
        }
    }

    /// Reads a stored name; one this version doesn't know becomes [`CalloutTone::Other`].
    #[must_use]
    pub fn parse(name: &str) -> Self {
        match name {
            "info" => Self::Info,
            "success" => Self::Success,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "note" => Self::Note,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CalloutTone;

    #[test]
    fn known_tones_round_trip_through_their_names() {
        for tone in CalloutTone::ALL {
            assert_eq!(CalloutTone::parse(tone.as_str()), tone);
        }
        let names: Vec<_> = CalloutTone::ALL.iter().map(CalloutTone::as_str).collect();
        assert_eq!(
            names,
            ["info", "success", "warning", "error", "note"],
            "stored names never change"
        );
    }

    #[test]
    fn unknown_names_are_kept() {
        let tone = CalloutTone::parse("celebrate");
        assert_eq!(tone, CalloutTone::Other("celebrate".into()));
        assert_eq!(tone.as_str(), "celebrate");
        assert_eq!(
            CalloutTone::parse("Info"),
            CalloutTone::Other("Info".into()),
            "names are exact"
        );
    }
}
