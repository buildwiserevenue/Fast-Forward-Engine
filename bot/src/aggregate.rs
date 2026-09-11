//! Aggregation logic for community compatibility reports (roadmap R4/R24).
//! Mirrors the private repo's `gui/crates/ffwd-bot/src/aggregate.rs`
//! sanity gates exactly (see that file's header for the full rationale):
//! BLOCKED is never touched in either direction, an existing VERIFIED
//! match is left alone, one report counts per (reporter, game), and a
//! suggested ceiling never regresses below one a profile already grants.
//! Nothing here writes `profiles.json` or its signature — only the
//! owner's local signing step can do that.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::profiles_lite::Db;
use crate::report::{SessionOutcome, SessionReport};

pub const MIN_REPORTERS: usize = 3;
pub const STRONG_REPORTERS: usize = 5;
pub const STRONG_CLEAN_RATIO: f64 = 0.9;
pub const MODERATE_CLEAN_RATIO: f64 = 0.7;

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
    pub report: SessionReport,
}

pub fn parse_all(
    raw: &[RawIssue],
    parser: impl Fn(&str) -> Option<SessionReport>,
) -> Vec<ParsedReport> {
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

pub fn aggregate(reports: &[ParsedReport], db: &Db) -> Vec<ProfileProposal> {
    let mut latest: HashMap<(String, String), &ParsedReport> = HashMap::new();
    for r in reports {
        let identity = r.report.identity_key();
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
            .entry(r.report.identity_key())
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

        let game_name = group
            .iter()
            .map(|r| r.report.game_name.as_str())
            .find(|n| !n.is_empty())
            .unwrap_or("(unnamed)")
            .to_string();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{AudioPolicySeen, DurationBucket, StatusSeen};

    fn report(exe: &str, outcome: SessionOutcome, common: f64) -> SessionReport {
        SessionReport {
            store: String::new(),
            store_id: String::new(),
            exe: exe.into(),
            game_name: "Test Game".into(),
            app_version: "0.1.0".into(),
            status_seen: StatusSeen::Unknown,
            factor_min: common - 0.5,
            factor_common: common,
            factor_max: common + 0.5,
            codec_events: 0,
            cutscene_speed_applied: None,
            audio_policy: AudioPolicySeen::None,
            outcome,
            duration: DurationBucket::FifteenToSixtyMin,
            notes: None,
            diagnostic_tail: None,
        }
    }

    fn parsed(author: &str, issue: u64, r: SessionReport) -> ParsedReport {
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
}
