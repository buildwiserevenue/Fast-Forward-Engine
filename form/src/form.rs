// Generated file: it is replaced whole on every update, so a change made here is not kept.

//! The community report's form: the ids of its fields, the words each field
//! may hold, and how a title's identity is written in it. The app writes a
//! report with these, and the aggregation bot reads one back with the same.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::ids::{valid_exe, valid_id};
use crate::store_kind::Store;

pub const FIELD_GAME_IDENTITY: &str = "game_identity";
pub const FIELD_GAME_NAME: &str = "game_name";
pub const FIELD_APP_VERSION: &str = "app_version";
pub const FIELD_STATUS_SEEN: &str = "status_seen";
pub const FIELD_FACTOR_MIN: &str = "factor_min";
pub const FIELD_FACTOR_COMMON: &str = "factor_common";
pub const FIELD_FACTOR_MAX: &str = "factor_max";
pub const FIELD_CODEC_EVENTS: &str = "codec_events";
pub const FIELD_CUTSCENE_SPEED: &str = "cutscene_speed_applied";
pub const FIELD_AUDIO_POLICY: &str = "audio_policy";
pub const FIELD_SESSION_OUTCOME: &str = "session_outcome";
pub const FIELD_DURATION: &str = "duration_bucket";
pub const FIELD_LAUNCH_ARGUMENT: &str = "launch_argument";
pub const FIELD_NOTES: &str = "notes";
pub const FIELD_DIAGNOSTIC: &str = "diagnostic_tail";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StatusSeen {
    Verified,
    Partial,
    Blocked,
    Unknown,
}

impl StatusSeen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "VERIFIED",
            Self::Partial => "PARTIAL",
            Self::Blocked => "BLOCKED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// The inverse of [`Self::as_str`]. Anything unrecognised is `Unknown`,
    /// never a guess at VERIFIED.
    pub fn parse(s: &str) -> Self {
        match s {
            "VERIFIED" => Self::Verified,
            "PARTIAL" => Self::Partial,
            "BLOCKED" => Self::Blocked,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AudioPolicySeen {
    None,
    Mute,
    Duck,
}

impl AudioPolicySeen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Mute => "mute",
            Self::Duck => "duck",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "mute" => Self::Mute,
            "duck" => Self::Duck,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SessionOutcome {
    CleanUnload,
    CrashOrKill,
}

impl SessionOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CleanUnload => "clean_unload",
            Self::CrashOrKill => "crash_or_kill",
        }
    }

    /// Ambiguous or unrecognised input is never counted as a clean run.
    pub fn parse(s: &str) -> Self {
        match s {
            "clean_unload" => Self::CleanUnload,
            _ => Self::CrashOrKill,
        }
    }
}

/// Coarse duration so a report never carries a precise, fingerprintable
/// second count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DurationBucket {
    UnderFiveMin,
    FiveToFifteenMin,
    FifteenToSixtyMin,
    OverSixtyMin,
}

impl DurationBucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnderFiveMin => "under_5m",
            Self::FiveToFifteenMin => "5_15m",
            Self::FifteenToSixtyMin => "15_60m",
            Self::OverSixtyMin => "over_60m",
        }
    }

    /// From a raw duration, for callers that only have seconds on hand.
    pub fn from_secs(secs: u64) -> Self {
        match secs {
            0..=299 => Self::UnderFiveMin,
            300..=899 => Self::FiveToFifteenMin,
            900..=3599 => Self::FifteenToSixtyMin,
            _ => Self::OverSixtyMin,
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "under_5m" => Self::UnderFiveMin,
            "5_15m" => Self::FiveToFifteenMin,
            "over_60m" => Self::OverSixtyMin,
            _ => Self::FifteenToSixtyMin,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// Neither a store identity nor an executable name.
    NoIdentity,
    /// A store or a value outside the shapes [`crate::ids`] allows.
    Malformed,
}

/// How a report names a title: a store's id that names it on every machine
/// (`steam:1091500`), else its executable's file name (`exe:Game.exe`). A
/// store id that names the title on one machine only is not used.
pub fn identity_key(store: &str, store_id: &str, exe: &str) -> Result<String, IdentityError> {
    if !store.is_empty() && !store_id.is_empty() {
        let store = Store::parse(store).ok_or(IdentityError::Malformed)?;
        if store.is_portable() {
            if !valid_id(store_id) {
                return Err(IdentityError::Malformed);
            }
            return Ok(format!("{}:{store_id}", store.as_str()));
        }
    }
    if exe.is_empty() {
        return Err(IdentityError::NoIdentity);
    }
    if !valid_exe(exe) {
        return Err(IdentityError::Malformed);
    }
    Ok(format!("exe:{exe}"))
}

/// The inverse of [`identity_key`]: `(store, store id, "")` or
/// `("", "", executable)`. Anything [`identity_key`] would not have written
/// is `None`.
pub fn parse_identity(raw: &str) -> Option<(String, String, String)> {
    let (kind, rest) = raw.split_once(':')?;
    let rest = rest.trim();
    if kind == "exe" {
        return valid_exe(rest).then(|| (String::new(), String::new(), rest.to_string()));
    }
    let store = Store::parse(kind).filter(|s| s.is_portable())?;
    valid_id(rest).then(|| (store.as_str().to_string(), rest.to_string(), String::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_named_by_a_portable_store_s_id_else_by_its_executable() {
        assert_eq!(
            identity_key("steam", "1091500", "x.exe"),
            Ok("steam:1091500".into())
        );
        assert_eq!(
            identity_key("gdk", "Pub.Title", ""),
            Ok("gdk:Pub.Title".into())
        );
        assert_eq!(
            identity_key("local", r"c:\games\my game\game.exe", "Game.exe"),
            Ok("exe:Game.exe".into())
        );
        assert_eq!(identity_key("", "", "Game.exe"), Ok("exe:Game.exe".into()));
        assert_eq!(
            identity_key("local", "x", ""),
            Err(IdentityError::NoIdentity)
        );
        assert_eq!(identity_key("", "", ""), Err(IdentityError::NoIdentity));
        assert_eq!(
            identity_key("itch", "1", "a.exe"),
            Err(IdentityError::Malformed)
        );
        assert_eq!(
            identity_key("steam", "1;drop", ""),
            Err(IdentityError::Malformed)
        );
        assert_eq!(
            identity_key("", "", "../a.exe"),
            Err(IdentityError::Malformed)
        );
    }

    #[test]
    fn what_is_read_back_is_only_what_could_have_been_written() {
        for (store, id, exe) in [
            ("steam", "42", ""),
            ("gdk", "Pub.Title", ""),
            ("", "", "Game.exe"),
        ] {
            let key = identity_key(store, id, exe).unwrap();
            assert_eq!(
                parse_identity(&key),
                Some((store.into(), id.into(), exe.into())),
                "{key}"
            );
        }
        for raw in [
            "itch:123",
            "steam:",
            ":123",
            "randomtext",
            "local:x.exe",
            "steam:[approved](https://evil.example)",
            "steam:42 extra",
            "epic:<img src=x>",
            "exe:../../.github/workflows/x.yml",
            "exe:C:\\x.exe",
            "exe:a\"b.exe",
            "exe:..",
        ] {
            assert_eq!(parse_identity(raw), None, "{raw}");
        }
    }

    #[test]
    fn duration_bucket_from_secs_covers_every_edge() {
        assert_eq!(DurationBucket::from_secs(0), DurationBucket::UnderFiveMin);
        assert_eq!(DurationBucket::from_secs(299), DurationBucket::UnderFiveMin);
        assert_eq!(
            DurationBucket::from_secs(300),
            DurationBucket::FiveToFifteenMin
        );
        assert_eq!(
            DurationBucket::from_secs(899),
            DurationBucket::FiveToFifteenMin
        );
        assert_eq!(
            DurationBucket::from_secs(900),
            DurationBucket::FifteenToSixtyMin
        );
        assert_eq!(
            DurationBucket::from_secs(3599),
            DurationBucket::FifteenToSixtyMin
        );
        assert_eq!(
            DurationBucket::from_secs(3600),
            DurationBucket::OverSixtyMin
        );
        assert_eq!(
            DurationBucket::from_secs(u64::MAX),
            DurationBucket::OverSixtyMin
        );
    }

    #[test]
    fn every_word_a_field_holds_is_read_back_as_itself() {
        for s in [
            StatusSeen::Verified,
            StatusSeen::Partial,
            StatusSeen::Blocked,
            StatusSeen::Unknown,
        ] {
            assert_eq!(StatusSeen::parse(s.as_str()), s);
        }
        for a in [
            AudioPolicySeen::None,
            AudioPolicySeen::Mute,
            AudioPolicySeen::Duck,
        ] {
            assert_eq!(AudioPolicySeen::parse(a.as_str()), a);
        }
        for o in [SessionOutcome::CleanUnload, SessionOutcome::CrashOrKill] {
            assert_eq!(SessionOutcome::parse(o.as_str()), o);
        }
        for d in [
            DurationBucket::UnderFiveMin,
            DurationBucket::FiveToFifteenMin,
            DurationBucket::FifteenToSixtyMin,
            DurationBucket::OverSixtyMin,
        ] {
            assert_eq!(DurationBucket::parse(d.as_str()), d);
        }
        assert_eq!(
            StatusSeen::parse("verified"),
            StatusSeen::Unknown,
            "never a guess"
        );
        assert_eq!(
            SessionOutcome::parse("clean"),
            SessionOutcome::CrashOrKill,
            "never counted clean"
        );
    }
}
