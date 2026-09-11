//! `ffwd-bot` — the community-report aggregator for this repo (roadmap
//! R4/R24 of the private FastForward Engine repo). Run only by
//! `.github/workflows/aggregate-reports.yml`, over an already-fetched batch
//! of open `compat-report` issues (`gh issue list ... > issues.json`). This
//! binary makes no GitHub API call and needs no token: it reads two local
//! files and writes local files under `pending/`, never `profiles.json` or
//! its signature. A human reviews every proposal in the pull request the
//! workflow opens; only the maintainer's local signing step can promote or
//! sign a profile.

mod aggregate;
mod parse;
mod profiles_lite;
mod report;

use std::path::PathBuf;
use std::process::ExitCode;

struct Args {
    issues: PathBuf,
    current_profiles: PathBuf,
    out_dir: PathBuf,
    pr_body_out: PathBuf,
}

fn parse_args() -> Option<Args> {
    let mut issues = None;
    let mut current_profiles = None;
    let mut out_dir = None;
    let mut pr_body_out = None;
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let val = it.next()?;
        match flag.as_str() {
            "--issues" => issues = Some(PathBuf::from(val)),
            "--current-profiles" => current_profiles = Some(PathBuf::from(val)),
            "--out-dir" => out_dir = Some(PathBuf::from(val)),
            "--pr-body-out" => pr_body_out = Some(PathBuf::from(val)),
            _ => return None,
        }
    }
    Some(Args {
        issues: issues?,
        current_profiles: current_profiles?,
        out_dir: out_dir?,
        pr_body_out: pr_body_out?,
    })
}

fn main() -> ExitCode {
    let Some(args) = parse_args() else {
        eprintln!(
            "usage: ffwd-bot --issues <issues.json> --current-profiles <profiles.json> \
             --out-dir <pending/> --pr-body-out <pr_body.md>"
        );
        return ExitCode::FAILURE;
    };

    let issues_json = match std::fs::read_to_string(&args.issues) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", args.issues.display());
            return ExitCode::FAILURE;
        }
    };
    let profiles_json = std::fs::read_to_string(&args.current_profiles).unwrap_or_default();
    let db = profiles_lite::Db::load(&profiles_json);

    let raw_issues: Vec<aggregate::RawIssue> = match serde_json::from_str(&issues_json) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("malformed issues file: {e}");
            return ExitCode::FAILURE;
        }
    };

    let reports = aggregate::parse_all(&raw_issues, parse::parse_issue_body);
    let proposals = aggregate::aggregate(&reports, &db);

    if proposals.is_empty() {
        println!("ffwd-bot: no proposal met the evidence bar — nothing written");
        return ExitCode::SUCCESS;
    }

    if let Err(e) = std::fs::create_dir_all(&args.out_dir) {
        eprintln!("cannot create {}: {e}", args.out_dir.display());
        return ExitCode::FAILURE;
    }
    let mut body = String::from(
        "Automated compatibility-evidence summary. This PR NEVER touches \
         `profiles.json` or `profiles.json.minisig` directly and proposes \
         nothing above PARTIAL — a human reviews every line, and only the \
         maintainer's local signing step can promote or sign a profile.\n\n",
    );
    for p in &proposals {
        let file_stem = p.identity.replace([':', '/'], "_");
        let path = args.out_dir.join(format!("{file_stem}.json"));
        let json = serde_json::to_string_pretty(p).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, json) {
            eprintln!("cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        body.push_str(&format!(
            "- **{}** ({}) — {} reporters, {:.0}% clean, factor {:.2}-{:.2}, \
             evidence: {:?} -> suggest PARTIAL at {:.2}x\n",
            p.game_name,
            p.identity,
            p.reporter_count,
            p.clean_ratio * 100.0,
            p.factor_low,
            p.factor_high,
            p.evidence,
            p.suggested_max_speed
        ));
    }
    if let Err(e) = std::fs::write(&args.pr_body_out, body) {
        eprintln!("cannot write {}: {e}", args.pr_body_out.display());
        return ExitCode::FAILURE;
    }
    println!("ffwd-bot: {} proposal(s) written", proposals.len());
    ExitCode::SUCCESS
}
