//! Turns many reports into proposals a maintainer reviews. BLOCKED is never
//! touched in either direction, an existing VERIFIED entry keeps its speed,
//! one report counts per (reporter, title), and a suggested ceiling never goes
//! below one the catalogue already grants. Launch options are counted apart:
//! at least [`MIN_LAUNCH_ARG_REPORTERS`] distinct reporters spelling the same
//! switch, BLOCKED never, VERIFIED allowed. Nothing here writes the catalogue
//! or its signature.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::parse::Report;
use crate::profiles_lite::Db;
use report_form::form::SessionOutcome;

pub const MIN_REPORTERS: usize = 3;
pub const STRONG_REPORTERS: usize = 5;
pub const STRONG_CLEAN_RATIO: f64 = 0.9;
pub const MODERATE_CLEAN_RATIO: f64 = 0.7;
/// Distinct reporters who must propose the SAME launch option, spelled the
/// same, for one title. Accounts are free: this filters noise, it does not
/// make a flag safe.
pub const MIN_LAUNCH_ARG_REPORTERS: usize = 10;

const MIN_SPEED: f64 = 0.1;
const MAX_SPEED: f64 = 10.0;

/// One row of `gh issue list --json number,author,body`.
#[derive(Debug, Clone, Deserialize)]
pub struct RawIssue {
    pub number: u64,
    pub author: Author,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Author {
    pub login: String,
    #[serde(default)]
    pub is_bot: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedReport {
    pub author: String,
    pub issue_number: u64,
    pub report: Report,
}

pub fn parse_all(raw: &[RawIssue], parser: impl Fn(&str) -> Option<Report>) -> Vec<ParsedReport> {
    raw.iter()
        .filter(|i| !i.author.is_bot)
        .filter_map(|i| {
            parser(&i.body).map(|report| ParsedReport {
                author: i.author.login.clone(),
                issue_number: i.number,
                report,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EvidenceStrength {
    Moderate,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileProposal {
    pub identity: String,
    pub game_name: String,
    pub reporter_count: usize,
    pub clean_ratio: f64,
    pub factor_low: f64,
    pub factor_high: f64,
    pub suggested_max_speed: f64,
    pub evidence: EvidenceStrength,
}

/// A launch option enough distinct reporters propose for one title. Never
/// signed here: it reaches a user only once the maintainer signs it, and then
/// only when that user switches it on at Play.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LaunchArgumentProposal {
    pub identity: String,
    pub game_name: String,
    pub launch_argument: String,
    pub reporter_count: usize,
    /// The flag the signed profile carries today, if it carries one.
    pub signed: Option<String>,
}

/// The name a group of reports gives its title: the first one a reporter
/// filled in.
fn first_name(group: &[&ParsedReport]) -> String {
    group
        .iter()
        .map(|r| r.report.game_name.as_str())
        .find(|n| !n.is_empty())
        .unwrap_or("(unnamed)")
        .to_string()
}

pub fn aggregate(reports: &[ParsedReport], db: &Db) -> Vec<ProfileProposal> {
    let mut latest: HashMap<(String, String), &ParsedReport> = HashMap::new();
    for r in reports {
        let identity = r.report.identity.clone();
        latest
            .entry((r.author.clone(), identity))
            .and_modify(|cur| {
                if r.issue_number > cur.issue_number {
                    *cur = r;
                }
            })
            .or_insert(r);
    }

    let mut by_identity: HashMap<String, Vec<&ParsedReport>> = HashMap::new();
    for r in latest.values() {
        by_identity
            .entry(r.report.identity.clone())
            .or_default()
            .push(r);
    }

    let mut out = Vec::new();
    for (identity, group) in by_identity {
        let sample = &group[0].report;
        let lookup = db.lookup(&sample.store, &sample.store_id, &sample.exe);
        if lookup.blocked || lookup.verified {
            continue;
        }

        let reporter_count = group.len();
        if reporter_count < MIN_REPORTERS {
            continue;
        }
        let clean = group
            .iter()
            .filter(|r| r.report.outcome == SessionOutcome::CleanUnload)
            .count();
        let clean_ratio = clean as f64 / reporter_count as f64;
        let evidence = if reporter_count >= STRONG_REPORTERS && clean_ratio >= STRONG_CLEAN_RATIO {
            EvidenceStrength::Strong
        } else if clean_ratio >= MODERATE_CLEAN_RATIO {
            EvidenceStrength::Moderate
        } else {
            continue;
        };

        let factor_low = group
            .iter()
            .map(|r| r.report.factor_min)
            .fold(f64::INFINITY, f64::min);
        let factor_high = group
            .iter()
            .map(|r| r.report.factor_max)
            .fold(0.0f64, f64::max);
        let mut commons: Vec<f64> = group.iter().map(|r| r.report.factor_common).collect();
        commons.sort_by(f64::total_cmp);
        let median = commons[commons.len() / 2];
        let suggested_max_speed = median
            .max(lookup.existing_ceiling)
            .clamp(MIN_SPEED, MAX_SPEED);

        let game_name = first_name(&group);

        out.push(ProfileProposal {
            identity,
            game_name,
            reporter_count,
            clean_ratio,
            factor_low,
            factor_high,
            suggested_max_speed,
            evidence,
        });
    }
    out.sort_by(|a, b| a.identity.cmp(&b.identity));
    out
}

/// Launch options proposed by at least [`MIN_LAUNCH_ARG_REPORTERS`] distinct
/// reporters for one title: one vote per (author, title), the author's newest
/// report that carries one; the exact spelling is the unit (whether a game
/// reads `-NoLogo` as `-nologo` is its own business); BLOCKED is never touched;
/// the flag the catalogue already carries is not news.
pub fn propose_launch_arguments(reports: &[ParsedReport], db: &Db) -> Vec<LaunchArgumentProposal> {
    let mut latest: HashMap<(String, String), (&ParsedReport, String)> = HashMap::new();
    for r in reports {
        let Some(arg) = r
            .report
            .launch_argument
            .as_deref()
            .and_then(report_form::launch_flags::clean_launch_argument)
        else {
            continue;
        };
        let slot = latest
            .entry((r.author.clone(), r.report.identity.clone()))
            .or_insert((r, arg.clone()));
        if r.issue_number > slot.0.issue_number {
            *slot = (r, arg);
        }
    }

    let mut by_flag: HashMap<(String, String), Vec<&ParsedReport>> = HashMap::new();
    for ((_, identity), (r, arg)) in latest {
        by_flag.entry((identity, arg)).or_default().push(r);
    }

    let mut out = Vec::new();
    for ((identity, launch_argument), group) in by_flag {
        if group.len() < MIN_LAUNCH_ARG_REPORTERS {
            continue;
        }
        let sample = &group[0].report;
        let lookup = db.lookup(&sample.store, &sample.store_id, &sample.exe);
        if lookup.blocked {
            continue;
        }
        if lookup.signed_launch_argument.as_deref() == Some(launch_argument.as_str()) {
            continue;
        }
        let game_name = first_name(&group);
        out.push(LaunchArgumentProposal {
            identity,
            game_name,
            launch_argument,
            reporter_count: group.len(),
            signed: lookup.signed_launch_argument,
        });
    }
    out.sort_by(|a, b| {
        (a.identity.as_str(), a.launch_argument.as_str())
            .cmp(&(b.identity.as_str(), b.launch_argument.as_str()))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(exe: &str, outcome: SessionOutcome, common: f64) -> Report {
        Report {
            identity: format!("exe:{exe}"),
            store: String::new(),
            store_id: String::new(),
            exe: exe.into(),
            game_name: "Test Game".into(),
            factor_min: common - 0.5,
            factor_common: common,
            factor_max: common + 0.5,
            outcome,
            launch_argument: None,
        }
    }

    /// `n` distinct reporters (`u1`..), each proposing `arg` for `exe`.
    fn proposing(exe: &str, arg: &str, n: u64, first_issue: u64) -> Vec<ParsedReport> {
        (0..n)
            .map(|i| {
                let mut r = report(exe, SessionOutcome::CleanUnload, 2.0);
                r.launch_argument = Some(arg.into());
                parsed(&format!("u{}", i + 1), first_issue + i, r)
            })
            .collect()
    }

    fn parsed(author: &str, issue: u64, r: Report) -> ParsedReport {
        ParsedReport {
            author: author.into(),
            issue_number: issue,
            report: r,
        }
    }

    #[test]
    fn strong_consistent_evidence_proposes_partial() {
        let reports: Vec<ParsedReport> = (1..=5)
            .map(|n| {
                parsed(
                    &format!("user{n}"),
                    n,
                    report("a.exe", SessionOutcome::CleanUnload, 2.0),
                )
            })
            .collect();
        let proposals = aggregate(&reports, &Db::default());
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].evidence, EvidenceStrength::Strong);
    }

    #[test]
    fn flooding_a_single_account_does_not_manufacture_evidence() {
        let reports: Vec<ParsedReport> = (1..=10)
            .map(|n| {
                parsed(
                    "flooder",
                    n,
                    report("a.exe", SessionOutcome::CleanUnload, 2.0),
                )
            })
            .collect();
        assert!(aggregate(&reports, &Db::default()).is_empty());
    }

    #[test]
    fn blocked_profile_is_never_proposed() {
        let db = Db::load(r#"[{"game_id":"g","exe_name":"a.exe","status":"BLOCKED"}]"#);
        let reports: Vec<ParsedReport> = (1..=5)
            .map(|n| {
                parsed(
                    &format!("u{n}"),
                    n,
                    report("a.exe", SessionOutcome::CleanUnload, 3.0),
                )
            })
            .collect();
        assert!(aggregate(&reports, &db).is_empty());
    }

    #[test]
    fn suggested_ceiling_never_regresses_below_an_existing_partial_profile() {
        let db = Db::load(
            r#"[{"game_id":"g","exe_name":"a.exe","status":"PARTIAL","max_recommended_speed":3.0}]"#,
        );
        let reports: Vec<ParsedReport> = (1..=5)
            .map(|n| {
                parsed(
                    &format!("u{n}"),
                    n,
                    report("a.exe", SessionOutcome::CleanUnload, 2.0),
                )
            })
            .collect();
        let proposals = aggregate(&reports, &db);
        assert_eq!(proposals.len(), 1);
        assert!((proposals[0].suggested_max_speed - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_launch_option_needs_ten_distinct_reporters() {
        let nine = proposing("a.exe", "-nologo", 9, 1);
        assert!(propose_launch_arguments(&nine, &Db::default()).is_empty());
        let p = propose_launch_arguments(&proposing("a.exe", "-nologo", 10, 1), &Db::default());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].launch_argument, "-nologo");
        assert_eq!(p[0].reporter_count, 10);
        assert_eq!(p[0].signed, None);
    }

    #[test]
    fn one_account_cannot_vote_a_launch_option_in() {
        let mut flood = proposing("a.exe", "-nologo", 1, 1);
        for n in 2..=30 {
            let mut r = report("a.exe", SessionOutcome::CleanUnload, 2.0);
            r.launch_argument = Some("-nologo".into());
            flood.push(parsed("u1", n, r));
        }
        assert!(propose_launch_arguments(&flood, &Db::default()).is_empty());
    }

    #[test]
    fn a_later_report_without_a_proposal_keeps_the_account_s_vote() {
        let mut reports = proposing("a.exe", "-nologo", 10, 1);
        reports.push(parsed(
            "u1",
            99,
            report("a.exe", SessionOutcome::CleanUnload, 2.0),
        ));
        assert_eq!(propose_launch_arguments(&reports, &Db::default()).len(), 1);
    }

    #[test]
    fn two_spellings_are_two_proposals_and_only_the_one_with_ten_goes_through() {
        let mut reports = proposing("a.exe", "-nologo", 10, 1);
        for i in 0..9 {
            let mut r = report("a.exe", SessionOutcome::CleanUnload, 2.0);
            r.launch_argument = Some("-NoLogo".into());
            reports.push(parsed(&format!("v{i}"), 100 + i, r));
        }
        let p = propose_launch_arguments(&reports, &Db::default());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].launch_argument, "-nologo");
    }

    #[test]
    fn a_launch_option_that_is_not_a_switch_is_never_counted() {
        let reports = proposing("a.exe", "-a;calc", 20, 1);
        assert!(propose_launch_arguments(&reports, &Db::default()).is_empty());
    }

    #[test]
    fn launch_options_skip_blocked_titles_but_not_verified_ones() {
        let blocked = Db::load(r#"[{"game_id":"b","exe_name":"a.exe","status":"BLOCKED"}]"#);
        assert!(
            propose_launch_arguments(&proposing("a.exe", "-nologo", 20, 1), &blocked).is_empty()
        );
        let verified = Db::load(
            r#"[{"game_id":"g","exe_name":"a.exe","status":"VERIFIED","max_recommended_speed":3.0}]"#,
        );
        assert_eq!(
            propose_launch_arguments(&proposing("a.exe", "-nologo", 10, 1), &verified).len(),
            1,
            "a VERIFIED title can still lack its flag"
        );
    }

    #[test]
    fn a_microsoft_store_title_is_matched_by_its_package() {
        let db = Db::load(
            r#"[{"game_id":"g","exe_name":"g.exe","gdk_package_id":"Pub.Title","status":"BLOCKED"}]"#,
        );
        let mut base = report("", SessionOutcome::CleanUnload, 2.0);
        base.identity = "gdk:Pub.Title".into();
        base.store = "gdk".into();
        base.store_id = "Pub.Title".into();
        base.launch_argument = Some("-nologo".into());
        let reports: Vec<ParsedReport> = (1..=10)
            .map(|n| parsed(&format!("u{n}"), n, base.clone()))
            .collect();
        assert!(propose_launch_arguments(&reports, &db).is_empty());
        assert_eq!(propose_launch_arguments(&reports, &Db::default()).len(), 1);
    }

    #[test]
    fn the_signed_flag_is_not_news_and_a_different_one_says_what_it_replaces() {
        let db = Db::load(
            r#"[{"game_id":"g","exe_name":"a.exe","status":"PARTIAL","launch_argument":"-nologo"}]"#,
        );
        assert!(propose_launch_arguments(&proposing("a.exe", "-nologo", 10, 1), &db).is_empty());
        let p = propose_launch_arguments(&proposing("a.exe", "-skipintro", 10, 1), &db);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].signed.as_deref(), Some("-nologo"));
    }
}
