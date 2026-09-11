//! VENDORED subset of the private FastForward Engine repo's
//! `gui/crates/ffwd-core/src/report.rs` — the `SessionReport` shape, its
//! field ids (must match `.github/ISSUE_TEMPLATE/compatibility-report.yml`
//! exactly) and the [`scrub`] redaction pass. The private repo is the
//! source of truth and carries the full test suite for this logic (encode
//! side included); this copy keeps only what the bot needs to PARSE and
//! AGGREGATE — never to build a share URL, which is an app-side concern.
//! If you change a field id or the schema on either side, change both
//! repos in the same sitting: a drift here silently breaks parsing of every
//! future report.

use serde::{Deserialize, Serialize};

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
pub const FIELD_NOTES: &str = "notes";
pub const FIELD_DIAGNOSTIC: &str = "diagnostic_tail";

pub const MAX_NOTES_CHARS: usize = 500;
pub const MAX_DIAGNOSTIC_CHARS: usize = 2000;

const REDACTED: &str = "<redacted-path>";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusSeen {
    Verified,
    Partial,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioPolicySeen {
    None,
    Mute,
    Duck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionOutcome {
    CleanUnload,
    CrashOrKill,
}

// The shared "Min" postfix mirrors the private repo's own field names
// (`under_5m`/`5_15m`/...) on purpose — this type is never glob-imported.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DurationBucket {
    UnderFiveMin,
    FiveToFifteenMin,
    FifteenToSixtyMin,
    OverSixtyMin,
}

/// A single session's compatibility evidence, as parsed back from a
/// rendered issue body. No pid, no path, no username, no machine id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionReport {
    #[serde(default)]
    pub store: String,
    #[serde(default)]
    pub store_id: String,
    /// Exe file NAME only (the weak identity rung) — never a path.
    #[serde(default)]
    pub exe: String,
    pub game_name: String,
    pub app_version: String,
    pub status_seen: StatusSeen,
    pub factor_min: f64,
    pub factor_common: f64,
    pub factor_max: f64,
    pub codec_events: u32,
    pub cutscene_speed_applied: Option<f64>,
    pub audio_policy: AudioPolicySeen,
    pub outcome: SessionOutcome,
    pub duration: DurationBucket,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub diagnostic_tail: Option<String>,
}

impl SessionReport {
    /// The grouping key: `"steam:1091500"` / `"exe:Foo.exe"`. Never
    /// validated here — `parse::split_identity` already refused anything
    /// malformed before a `SessionReport` exists.
    pub fn identity_key(&self) -> String {
        if !self.store.is_empty() && !self.store_id.is_empty() {
            format!("{}:{}", self.store, self.store_id)
        } else {
            format!("exe:{}", self.exe)
        }
    }
}

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

fn starts_with_ci(chars: &[char], at: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    at + w.len() <= chars.len()
        && chars[at..at + w.len()]
            .iter()
            .zip(&w)
            .all(|(&c, &wc)| c.to_ascii_lowercase() == wc)
}

/// Redact anything that looks like a Windows or Linux user-profile path
/// segment. Byte-for-byte the same algorithm as the private repo's
/// `ffwd_core::report::scrub` — see that file's doc comment for the full
/// design rationale and the adversarial test cases proving it.
pub fn scrub(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < n {
        let boundary_before = i == 0 || is_sep(chars[i - 1]);
        let hit = if boundary_before {
            if starts_with_ci(&chars, i, "users") {
                Some(i + 5)
            } else if starts_with_ci(&chars, i, "home") {
                Some(i + 4)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(after_word) = hit
            && after_word < n
            && is_sep(chars[after_word])
        {
            let mut j = after_word + 1;
            while j < n && !is_sep(chars[j]) {
                j += 1;
            }
            out.push_str(REDACTED);
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn clean_notes(raw: &str) -> Option<String> {
    let s = scrub(raw.trim());
    let s: String = s.chars().take(MAX_NOTES_CHARS).collect();
    (!s.is_empty()).then_some(s)
}

pub fn clean_diagnostic_tail(raw: &str) -> Option<String> {
    let s = scrub(raw.trim());
    let s: String = s.chars().take(MAX_DIAGNOSTIC_CHARS).collect();
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_redacts_windows_and_linux_profile_segments() {
        assert_eq!(
            scrub(r"crash in C:\Users\Giovanni Bianchi\Documents\save.dat"),
            r"crash in C:\<redacted-path>\Documents\save.dat"
        );
        assert_eq!(
            scrub("/home/billy/projects/foo"),
            "/<redacted-path>/projects/foo"
        );
    }

    #[test]
    fn scrub_leaves_ordinary_prose_untouched() {
        assert_eq!(
            scrub("Many Users play this slowly"),
            "Many Users play this slowly"
        );
    }

    #[test]
    fn identity_key_prefers_store_over_exe() {
        let mut r = sample();
        r.store = "steam".into();
        r.store_id = "42".into();
        r.exe = "Foo.exe".into();
        assert_eq!(r.identity_key(), "steam:42");
        r.store.clear();
        r.store_id.clear();
        assert_eq!(r.identity_key(), "exe:Foo.exe");
    }

    fn sample() -> SessionReport {
        SessionReport {
            store: String::new(),
            store_id: String::new(),
            exe: "a.exe".into(),
            game_name: "A".into(),
            app_version: "0.1.0".into(),
            status_seen: StatusSeen::Unknown,
            factor_min: 1.0,
            factor_common: 1.0,
            factor_max: 1.0,
            codec_events: 0,
            cutscene_speed_applied: None,
            audio_policy: AudioPolicySeen::None,
            outcome: SessionOutcome::CleanUnload,
            duration: DurationBucket::UnderFiveMin,
            notes: None,
            diagnostic_tail: None,
        }
    }
}
