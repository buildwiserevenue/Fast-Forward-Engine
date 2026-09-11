//! Minimal reader over THIS repo's own `profiles.json`
//! (`docs/architecture/profiles-schema.md` in the private repo is the
//! contract). Deliberately NOT the full matching ladder the shipped app
//! uses (`ffwd_core::profiles::match_game`) — the bot only needs a
//! conservative yes/no answer to two questions, and answering them more
//! simply is easier to keep correct in a vendored copy than replicating the
//! full ladder would be:
//!   - is this identity blocked (by status or the anti-cheat flag), through
//!     EITHER its store id OR its exe name? If either says yes, skip it.
//!   - is it already VERIFIED through either rung? If yes, leave it alone
//!     (v1 never touches an existing VERIFIED entry).
//!
//! Being more conservative than the real ladder is the safe direction for
//! a bot that can only ever propose, never decide.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub exe_name: String,
    #[serde(default)]
    pub steam_appid: u64,
    #[serde(default)]
    pub epic_catalog_item_id: String,
    #[serde(default)]
    pub gog_product_id: u64,
    pub status: String,
    #[serde(default)]
    pub max_recommended_speed: f64,
    #[serde(default)]
    pub anti_cheat_detected: bool,
}

impl Profile {
    fn is_blocked(&self) -> bool {
        self.anti_cheat_detected || self.status == "BLOCKED"
    }

    fn matches(&self, store: &str, store_id: &str, exe: &str) -> bool {
        if !exe.is_empty() && self.exe_name.eq_ignore_ascii_case(exe) {
            return true;
        }
        if store.is_empty() || store_id.is_empty() {
            return false;
        }
        match store {
            "steam" => store_id
                .parse::<u64>()
                .is_ok_and(|id| id == self.steam_appid),
            "epic" => self.epic_catalog_item_id == store_id,
            "gog" => store_id
                .parse::<u64>()
                .is_ok_and(|id| id == self.gog_product_id),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Db {
    profiles: Vec<Profile>,
}

pub struct Lookup {
    pub blocked: bool,
    pub verified: bool,
    /// The highest ceiling any matching profile already grants; `1.0` when
    /// no profile exists for this identity at all.
    pub existing_ceiling: f64,
}

impl Db {
    /// Never fails the run: a missing or malformed `profiles.json` is
    /// treated as an empty catalogue, which is the conservative direction
    /// (nothing is "already verified", nothing is "already blocked" —
    /// every proposal still starts from PARTIAL, never above it).
    pub fn load(json: &str) -> Db {
        let profiles = serde_json::from_str(json).unwrap_or_default();
        Db { profiles }
    }

    pub fn lookup(&self, store: &str, store_id: &str, exe: &str) -> Lookup {
        let matches: Vec<&Profile> = self
            .profiles
            .iter()
            .filter(|p| p.matches(store, store_id, exe))
            .collect();
        Lookup {
            blocked: matches.iter().any(|p| p.is_blocked()),
            verified: matches
                .iter()
                .any(|p| p.status == "VERIFIED" && !p.is_blocked()),
            existing_ceiling: matches
                .iter()
                .filter(|p| !p.is_blocked())
                .map(|p| p.max_recommended_speed)
                .fold(1.0, f64::max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Db {
        Db::load(
            r#"[
                {"game_id":"a","exe_name":"blocked.exe","status":"BLOCKED","anti_cheat_detected":true},
                {"game_id":"b","exe_name":"verified.exe","steam_appid":42,"status":"VERIFIED","max_recommended_speed":3.0},
                {"game_id":"c","exe_name":"partial.exe","status":"PARTIAL","max_recommended_speed":2.0}
            ]"#,
        )
    }

    #[test]
    fn blocked_by_exe_or_store_id() {
        let db = sample();
        assert!(db.lookup("", "", "blocked.exe").blocked);
        assert!(!db.lookup("", "", "unknown.exe").blocked);
    }

    #[test]
    fn verified_only_through_a_non_blocked_match() {
        let db = sample();
        assert!(db.lookup("steam", "42", "anything.exe").verified);
        assert!(db.lookup("", "", "verified.exe").verified);
        assert!(!db.lookup("", "", "blocked.exe").verified);
    }

    #[test]
    fn existing_ceiling_defaults_to_one_when_unknown() {
        let db = sample();
        assert_eq!(db.lookup("", "", "unknown.exe").existing_ceiling, 1.0);
        assert_eq!(db.lookup("", "", "partial.exe").existing_ceiling, 2.0);
    }

    #[test]
    fn malformed_json_degrades_to_an_empty_catalogue_not_a_crash() {
        let db = Db::load("not json");
        let l = db.lookup("steam", "1", "a.exe");
        assert!(!l.blocked && !l.verified);
        assert_eq!(l.existing_ceiling, 1.0);
    }
}
