//! Content-addressed IDs for files such as images.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::CoreError;

/// Identifies an asset by the BLAKE3 hash of its bytes.
///
/// Naming files by content means the same image pasted twice is stored once, and a file on disk
/// can always be checked against its name. Shown and stored as 64 lowercase hex characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId([u8; 32]);

impl AssetId {
    /// The ID of these bytes. Hashing the same bytes always gives the same ID.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    /// The raw 32-byte hash.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&blake3::Hash::from_bytes(self.0).to_hex(), f)
    }
}

impl FromStr for AssetId {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = |reason: String| CoreError::InvalidId {
            kind: "asset",
            input: s.to_owned(),
            reason,
        };
        // blake3 also accepts uppercase; we only write lowercase, so only accept that and keep one spelling.
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(invalid("hex must be lowercase".to_owned()));
        }
        blake3::Hash::from_hex(s)
            .map(|hash| Self(*hash.as_bytes()))
            .map_err(|e| invalid(e.to_string()))
    }
}

impl Serialize for AssetId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for AssetId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::AssetId;
    use crate::CoreError;

    /// BLAKE3 of empty input, from the BLAKE3 reference test vectors.
    const EMPTY: &str = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";

    #[test]
    fn same_bytes_same_id_and_known_vector() {
        assert_eq!(AssetId::of(b"").to_string(), EMPTY);
        assert_eq!(AssetId::of(b"leaf"), AssetId::of(b"leaf"));
        assert_ne!(AssetId::of(b"leaf"), AssetId::of(b"leaf!"));
    }

    #[test]
    fn display_and_parse_round_trip() {
        let id = AssetId::of(b"an image");
        assert_eq!(id.to_string().parse::<AssetId>(), Ok(id));
        assert_eq!(id.as_bytes().len(), 32);
    }

    #[test]
    fn parsing_rejects_bad_hex() {
        for bad in [
            "",
            "abc",
            &EMPTY.to_uppercase(),
            &format!("{EMPTY}00"),
            &EMPTY.replace('a', "g"),
        ] {
            let err = bad.parse::<AssetId>().unwrap_err();
            assert!(
                matches!(err, CoreError::InvalidId { kind: "asset", .. }),
                "{bad:?} gave {err}"
            );
        }
    }

    #[test]
    fn serde_round_trip_uses_hex() {
        let id = AssetId::of(b"x");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<AssetId>(&json).unwrap(), id);
        assert!(serde_json::from_str::<AssetId>("\"zz\"").is_err());
        assert!(serde_json::from_str::<AssetId>("42").is_err());
    }
}
