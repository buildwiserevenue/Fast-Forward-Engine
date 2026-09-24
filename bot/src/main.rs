//! The community-report aggregator of this repository, run only by
//! `.github/workflows/aggregate-reports.yml` over an already-fetched batch of
//! open `compat-report` issues. It makes no network call and needs no token:
//! it reads two local files and writes proposals under `pending/`, never
//! `profiles.json` or its signature. A maintainer reviews every proposal in the
//! pull request the workflow opens, and signs the catalogue themselves.

mod aggregate;
mod parse;
mod profiles_lite;

use std::path::PathBuf;
use std::process::ExitCode;

/// Longest reporter text the pull request shows.
const MAX_SHOWN: usize = 64;

/// The pull request's body when no report meets the evidence bar.
const NO_PROPOSAL: &str = "No open report meets the evidence bar: nothing to review.\n";

/// A reporter's text as the pull request shows it: inside a code span, so GitHub
/// renders it literally — never a link, an image, a mention or markup — with what
/// could close the span or disguise the text (backticks, line breaks, bidirectional
/// and invisible format characters) taken out, and bounded. Anyone can open an
/// issue here, and the maintainer reads this body before signing anything.
fn inert(text: &str) -> String {
    let clean: String = text
        .chars()
        .filter(|&c| c != '`' && !c.is_control() && !is_format(c))
        .take(MAX_SHOWN)
        .collect();
    let clean = clean.trim();
    format!("`{}`", if clean.is_empty() { "(unnamed)" } else { clean })
}

/// Invisible characters that reorder or hide text on screen.
fn is_format(c: char) -> bool {
    matches!(c,
        '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}'
        | '\u{FEFF}')
}

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
            "usage: report-bot --issues <issues.json> --current-profiles <profiles.json> \
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
    let launch_args = aggregate::propose_launch_arguments(&reports, &db);

    if proposals.is_empty() && launch_args.is_empty() {
        // The workflow runs its pull-request step anyway: with no proposal
        // file the branch stops differing from main and the open pull request
        // is closed. That step still reads a body.
        if let Err(e) = std::fs::write(&args.pr_body_out, NO_PROPOSAL) {
            eprintln!("cannot write {}: {e}", args.pr_body_out.display());
            return ExitCode::FAILURE;
        }
        println!("report-bot: no proposal met the evidence bar — no proposal written");
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
        if let Err(e) = write_proposal(&args.out_dir, &p.identity, ".json", p) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        body.push_str(&format!(
            "- {} ({}) — {} reporters, {:.0}% clean, factor {:.2}-{:.2}, \
             evidence: {:?} -> suggest PARTIAL at {:.2}x\n",
            inert(&p.game_name),
            inert(&p.identity),
            p.reporter_count,
            p.clean_ratio * 100.0,
            p.factor_low,
            p.factor_high,
            p.evidence,
            p.suggested_max_speed
        ));
    }
    if !launch_args.is_empty() {
        body.push_str(&format!(
            "\nLaunch options proposed by at least {} distinct reporters each. \
             None reaches a user until the maintainer signs it, and then only \
             for a user who switches it on at Play:\n\n",
            aggregate::MIN_LAUNCH_ARG_REPORTERS
        ));
    }
    for p in &launch_args {
        if let Err(e) = write_proposal(&args.out_dir, &p.identity, ".launch.json", p) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        body.push_str(&launch_arg_line(p));
    }
    if let Err(e) = std::fs::write(&args.pr_body_out, body) {
        eprintln!("cannot write {}: {e}", args.pr_body_out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "report-bot: {} proposal(s) and {} launch option(s) written",
        proposals.len(),
        launch_args.len()
    );
    ExitCode::SUCCESS
}

/// One proposal as a file under `pending/`: the title's identity made a file
/// name the same way for every kind of proposal, `suffix` telling the kinds
/// apart.
fn write_proposal(
    out_dir: &std::path::Path,
    identity: &str,
    suffix: &str,
    proposal: &impl serde::Serialize,
) -> Result<(), String> {
    let path = out_dir.join(format!("{}{suffix}", identity.replace([':', '/'], "_")));
    let json = serde_json::to_string_pretty(proposal)
        .map_err(|e| format!("cannot encode {}: {e}", path.display()))?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// One launch-option proposal as the pull request lists it: every value a
/// reporter could have written is shown [`inert`].
fn launch_arg_line(p: &aggregate::LaunchArgumentProposal) -> String {
    format!(
        "- {} ({}) — {} reporters propose {}; signed today: {}\n",
        inert(&p.game_name),
        inert(&p.identity),
        p.reporter_count,
        inert(&p.launch_argument),
        p.signed
            .as_deref()
            .map_or_else(|| "none".to_string(), inert),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reporter_s_text_reaches_the_pull_request_inert() {
        // A link, an image, a mention and a heading stay literal inside a code span.
        assert_eq!(
            inert("[approved by maintainer](https://evil.example)"),
            "`[approved by maintainer](https://evil.example)`"
        );
        // Nothing can close the span early, start a new line or turn the text around.
        assert_eq!(inert("a`[x](y)`b"), "`a[x](y)b`");
        assert_eq!(
            inert("Game\n## Signed by the maintainer"),
            "`Game## Signed by the maintainer`"
        );
        assert_eq!(inert("\u{202E}exe.gnp"), "`exe.gnp`");
        assert_eq!(inert("a\u{200B}b"), "`ab`");
        // Bounded, and never empty.
        assert_eq!(inert(&"x".repeat(500)).len(), MAX_SHOWN + 2);
        assert_eq!(inert(" \u{200B} "), "`(unnamed)`");
        // Positive control: an ordinary name is shown as it is.
        assert_eq!(inert("Cyberpunk 2077"), "`Cyberpunk 2077`");
    }

    #[test]
    fn a_launch_option_line_shows_every_reporter_value_inert() {
        let p = aggregate::LaunchArgumentProposal {
            identity: "steam:42".into(),
            game_name: "[Signed](https://evil.example) @maintainer".into(),
            launch_argument: "-nologo".into(),
            reporter_count: 12,
            signed: None,
        };
        assert_eq!(
            launch_arg_line(&p),
            "- `[Signed](https://evil.example) @maintainer` (`steam:42`) — 12 reporters \
             propose `-nologo`; signed today: none\n"
        );
        let p = aggregate::LaunchArgumentProposal {
            signed: Some("-skipintro".into()),
            ..p
        };
        assert!(launch_arg_line(&p).ends_with("signed today: `-skipintro`\n"));
    }
}
