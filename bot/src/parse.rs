//! Reads a rendered issue form back into the few values the aggregation uses.
//!
//! GitHub renders every form field as an `### <label>` heading followed by its
//! value, `_No response_` for an optional field left empty; the form gives each
//! field a label identical to its id, so a heading is the id the bot looks for.
//! Anyone can type into an issue: a value outside its field's shape is dropped,
//! never repaired, and a report that names no title is no report.

use report_form::form::{self, SessionOutcome};
use report_form::launch_flags::clean_launch_argument;

const NO_RESPONSE: &str = "_No response_";

/// What the bot reads from one report: only what the aggregation uses.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// How the form names the title (`steam:42`, `exe:Game.exe`).
    pub identity: String,
    pub store: String,
    pub store_id: String,
    pub exe: String,
    pub game_name: String,
    pub factor_min: f64,
    pub factor_common: f64,
    pub factor_max: f64,
    pub outcome: SessionOutcome,
    /// A proposed launch option, kept only when it is a single switch.
    pub launch_argument: Option<String>,
}

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

fn number(s: Option<&str>, default: f64) -> f64 {
    s.and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|f| f.is_finite())
        .unwrap_or(default)
}

/// `None` only when the report names no title the form could have written;
/// any other field degrades to a conservative default, so one malformed value
/// never discards the rest of the evidence.
pub fn parse_issue_body(body: &str) -> Option<Report> {
    let sections = split_sections(body);
    let (store, store_id, exe) = form::parse_identity(get(&sections, form::FIELD_GAME_IDENTITY)?)?;
    let identity = form::identity_key(&store, &store_id, &exe).ok()?;
    Some(Report {
        identity,
        store,
        store_id,
        exe,
        game_name: get(&sections, form::FIELD_GAME_NAME)
            .unwrap_or_default()
            .to_string(),
        factor_min: number(get(&sections, form::FIELD_FACTOR_MIN), 1.0),
        factor_common: number(get(&sections, form::FIELD_FACTOR_COMMON), 1.0),
        factor_max: number(get(&sections, form::FIELD_FACTOR_MAX), 1.0),
        outcome: SessionOutcome::parse(
            get(&sections, form::FIELD_SESSION_OUTCOME).unwrap_or_default(),
        ),
        launch_argument: get(&sections, form::FIELD_LAUNCH_ARGUMENT)
            .and_then(clean_launch_argument),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(fields: &[(&str, &str)]) -> String {
        fields
            .iter()
            .map(|(k, v)| format!("### {k}\n\n{v}\n\n"))
            .collect()
    }

    #[test]
    fn reads_what_the_form_renders() {
        let r = parse_issue_body(&body(&[
            (form::FIELD_GAME_IDENTITY, "steam:1091500"),
            (form::FIELD_GAME_NAME, "Cyberpunk 2077"),
            (form::FIELD_FACTOR_MIN, "1.5"),
            (form::FIELD_FACTOR_COMMON, "2"),
            (form::FIELD_FACTOR_MAX, "3"),
            (form::FIELD_SESSION_OUTCOME, "clean_unload"),
            (form::FIELD_LAUNCH_ARGUMENT, "-NoStartupMovies"),
            (form::FIELD_NOTES, NO_RESPONSE),
        ]))
        .unwrap();
        assert_eq!(r.identity, "steam:1091500");
        assert_eq!(
            (r.store.as_str(), r.store_id.as_str()),
            ("steam", "1091500")
        );
        assert_eq!(r.game_name, "Cyberpunk 2077");
        assert_eq!(
            (r.factor_min, r.factor_common, r.factor_max),
            (1.5, 2.0, 3.0)
        );
        assert_eq!(r.outcome, SessionOutcome::CleanUnload);
        assert_eq!(r.launch_argument.as_deref(), Some("-NoStartupMovies"));
    }

    #[test]
    fn a_report_that_names_no_title_the_form_could_write_is_none() {
        assert!(parse_issue_body("").is_none());
        assert!(parse_issue_body("garbage with no headings at all").is_none());
        assert!(parse_issue_body(&body(&[(form::FIELD_GAME_NAME, "Foo")])).is_none());
        for raw in [
            "itch:1",
            "local:x.exe",
            "steam:[approved](https://evil.example)",
            "exe:../x.exe",
        ] {
            assert!(
                parse_issue_body(&body(&[(form::FIELD_GAME_IDENTITY, raw)])).is_none(),
                "{raw}"
            );
        }
        let gdk = parse_issue_body(&body(&[(form::FIELD_GAME_IDENTITY, "gdk:Pub.Title")])).unwrap();
        assert_eq!(gdk.identity, "gdk:Pub.Title");
    }

    #[test]
    fn a_malformed_value_degrades_and_never_counts_as_clean() {
        let r = parse_issue_body(&body(&[
            (form::FIELD_GAME_IDENTITY, "exe:Foo.exe"),
            (form::FIELD_FACTOR_MIN, "not-a-number"),
            (form::FIELD_SESSION_OUTCOME, "weird"),
        ]))
        .unwrap();
        assert_eq!(r.factor_min, 1.0);
        assert_eq!(r.outcome, SessionOutcome::CrashOrKill);
    }

    #[test]
    fn a_launch_option_that_is_not_a_single_switch_is_dropped_not_repaired() {
        for attack in [
            "-a;calc",
            "-nologo & powershell -enc AAAA",
            "+exec autoexec.cfg",
            "-ExecCmds=quit",
            "[-nologo](https://evil.example)",
            "`-nologo`",
            "-a\u{202E}b",
            NO_RESPONSE,
        ] {
            let r = parse_issue_body(&body(&[
                (form::FIELD_GAME_IDENTITY, "exe:a.exe"),
                (form::FIELD_LAUNCH_ARGUMENT, attack),
            ]))
            .unwrap();
            assert_eq!(r.launch_argument, None, "{attack:?}");
            assert_eq!(r.exe, "a.exe", "the rest of the report still counts");
        }
    }

    #[test]
    fn handles_crlf_and_a_trailing_heading_with_no_value() {
        let r = parse_issue_body("\r\n### game_identity\r\n\r\nexe:a.exe\r\n\r\n### notes\r\n")
            .unwrap();
        assert_eq!(r.exe, "a.exe");
    }
}
