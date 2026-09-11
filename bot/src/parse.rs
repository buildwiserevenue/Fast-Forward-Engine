//! Parses a rendered GitHub Issue Form body back into a [`SessionReport`].
//! Mirrors the private repo's `gui/crates/ffwd-bot/src/parse.rs` — see that
//! file's header for why this hand-rolled parser exists instead of a JS
//! issue-parsing action (one language, one implementation, host-tested).
//!
//! `compatibility-report.yml` gives every field a `label` IDENTICAL to its
//! `id`: a heading can then never drift from the id this parser looks for.

use crate::report::{
    self, AudioPolicySeen, DurationBucket, SessionOutcome, SessionReport, StatusSeen,
};

const NO_RESPONSE: &str = "_No response_";

fn split_sections(body: &str) -> Vec<(String, String)> {
    let normalized = body.replace("\r\n", "\n");
    let padded = format!("\n{normalized}");
    let mut out = Vec::new();
    for chunk in padded.split("\n### ").skip(1) {
        let (label, value) = chunk.split_once("\n\n").unwrap_or((chunk, ""));
        let label = label.trim().to_string();
        let mut value = value.trim().to_string();
        if value == NO_RESPONSE {
            value.clear();
        }
        out.push((label, value));
    }
    out
}

fn get<'a>(sections: &'a [(String, String)], id: &str) -> Option<&'a str> {
    sections
        .iter()
        .find(|(l, _)| l == id)
        .map(|(_, v)| v.as_str())
}

fn split_identity(raw: &str) -> Option<(String, String, String)> {
    let (kind, rest) = raw.split_once(':')?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    match kind {
        "steam" | "epic" | "gog" => Some((kind.to_string(), rest.to_string(), String::new())),
        "exe" => Some((String::new(), String::new(), rest.to_string())),
        _ => None,
    }
}

fn parse_status(s: &str) -> StatusSeen {
    match s {
        "VERIFIED" => StatusSeen::Verified,
        "PARTIAL" => StatusSeen::Partial,
        "BLOCKED" => StatusSeen::Blocked,
        _ => StatusSeen::Unknown,
    }
}

fn parse_audio(s: &str) -> AudioPolicySeen {
    match s {
        "mute" => AudioPolicySeen::Mute,
        "duck" => AudioPolicySeen::Duck,
        _ => AudioPolicySeen::None,
    }
}

fn parse_outcome(s: &str) -> SessionOutcome {
    match s {
        "clean_unload" => SessionOutcome::CleanUnload,
        _ => SessionOutcome::CrashOrKill,
    }
}

fn parse_duration(s: &str) -> DurationBucket {
    match s {
        "under_5m" => DurationBucket::UnderFiveMin,
        "5_15m" => DurationBucket::FiveToFifteenMin,
        "over_60m" => DurationBucket::OverSixtyMin,
        _ => DurationBucket::FifteenToSixtyMin,
    }
}

fn parse_f64(s: Option<&str>, default: f64) -> f64 {
    s.and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|f| f.is_finite())
        .unwrap_or(default)
}

pub fn parse_issue_body(body: &str) -> Option<SessionReport> {
    let sections = split_sections(body);
    let (store, store_id, exe) = split_identity(get(&sections, report::FIELD_GAME_IDENTITY)?)?;
    let cutscene = match get(&sections, report::FIELD_CUTSCENE_SPEED) {
        Some("none") | None | Some("") => None,
        Some(v) => v.trim().parse::<f64>().ok().filter(|f| f.is_finite()),
    };
    Some(SessionReport {
        store,
        store_id,
        exe,
        game_name: get(&sections, report::FIELD_GAME_NAME)
            .unwrap_or_default()
            .to_string(),
        app_version: get(&sections, report::FIELD_APP_VERSION)
            .unwrap_or_default()
            .to_string(),
        status_seen: parse_status(get(&sections, report::FIELD_STATUS_SEEN).unwrap_or_default()),
        factor_min: parse_f64(get(&sections, report::FIELD_FACTOR_MIN), 1.0),
        factor_common: parse_f64(get(&sections, report::FIELD_FACTOR_COMMON), 1.0),
        factor_max: parse_f64(get(&sections, report::FIELD_FACTOR_MAX), 1.0),
        codec_events: get(&sections, report::FIELD_CODEC_EVENTS)
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(0),
        cutscene_speed_applied: cutscene,
        audio_policy: parse_audio(get(&sections, report::FIELD_AUDIO_POLICY).unwrap_or_default()),
        outcome: parse_outcome(get(&sections, report::FIELD_SESSION_OUTCOME).unwrap_or_default()),
        duration: parse_duration(get(&sections, report::FIELD_DURATION).unwrap_or_default()),
        notes: get(&sections, report::FIELD_NOTES).and_then(report::clean_notes),
        diagnostic_tail: get(&sections, report::FIELD_DIAGNOSTIC)
            .and_then(report::clean_diagnostic_tail),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_realistic_rendered_body() {
        let body = format!(
            "### {}\n\nexe:Cyberpunk2077.exe\n\n### {}\n\nCyberpunk 2077\n\n### {}\n\n0.1.0\n\n\
             ### {}\n\nPARTIAL\n\n### {}\n\n1.50\n\n### {}\n\n2\n\n### {}\n\n3\n\n\
             ### {}\n\n0\n\n### {}\n\nnone\n\n### {}\n\nnone\n\n### {}\n\nclean_unload\n\n\
             ### {}\n\n15_60m\n\n### {}\n\n_No response_\n",
            report::FIELD_GAME_IDENTITY,
            report::FIELD_GAME_NAME,
            report::FIELD_APP_VERSION,
            report::FIELD_STATUS_SEEN,
            report::FIELD_FACTOR_MIN,
            report::FIELD_FACTOR_COMMON,
            report::FIELD_FACTOR_MAX,
            report::FIELD_CODEC_EVENTS,
            report::FIELD_CUTSCENE_SPEED,
            report::FIELD_AUDIO_POLICY,
            report::FIELD_SESSION_OUTCOME,
            report::FIELD_DURATION,
            report::FIELD_NOTES,
        );
        let r = parse_issue_body(&body).unwrap();
        assert_eq!(r.exe, "Cyberpunk2077.exe");
        assert_eq!(r.game_name, "Cyberpunk 2077");
        assert_eq!(r.status_seen, StatusSeen::Partial);
        assert_eq!(r.outcome, SessionOutcome::CleanUnload);
    }

    #[test]
    fn missing_or_unknown_identity_yields_no_report() {
        assert!(parse_issue_body("").is_none());
        assert!(
            parse_issue_body(&format!("### {}\n\nitch:1\n", report::FIELD_GAME_IDENTITY)).is_none()
        );
    }

    #[test]
    fn malformed_fields_degrade_conservatively() {
        let body = format!(
            "### {}\n\nexe:Foo.exe\n\n### {}\n\nweird\n",
            report::FIELD_GAME_IDENTITY,
            report::FIELD_SESSION_OUTCOME,
        );
        let r = parse_issue_body(&body).unwrap();
        assert_eq!(r.outcome, SessionOutcome::CrashOrKill);
    }
}
