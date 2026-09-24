// Generated file: it is replaced whole on every update, so a change made here is not kept.

//! The one shape a launch option may take to skip a title's startup logos or
//! intros: a SWITCH — a name, at most a number. It can carry neither a second
//! argument, a redirection or a shell command, nor what a game itself reads as
//! a command or a place: a console line (`+quit`), a value naming a command, a
//! file, a path or an address (`-ExecCmds=quit`, `-log=../x`,
//! `-connect=1.2.3.4:27015`). The same rule holds for every title: no list of
//! titles or engines exists here. What no shape can know — a switch whose
//! name alone does harm in one game — is left to human review.

/// Longest accepted argument, whole.
const MAX_LEN: usize = 64;

/// A launch argument is safe to append to a game's own launch only as a
/// switch: one or two dashes, a name that starts with a letter and holds only
/// letters, digits, `_` and `-`, and at most `=` followed by digits. This is
/// the ONE rule, applied to every profile's flag identically — never a
/// per-title decision.
pub fn is_safe_launch_arg(arg: &str) -> bool {
    let Some(body) = arg.strip_prefix('-') else {
        return false;
    };
    let body = body.strip_prefix('-').unwrap_or(body);
    let (name, value) = match body.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (body, None),
    };
    arg.len() <= MAX_LEN
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        && value.is_none_or(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()))
}

/// A proposed launch option, surrounding whitespace dropped, when it is a
/// switch; anything else is dropped whole, never repaired into one.
pub fn clean_launch_argument(raw: &str) -> Option<String> {
    let arg = raw.trim();
    is_safe_launch_arg(arg).then(|| arg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_from_any_engine_is_accepted() {
        // The rule knows nothing about which game or engine these belong to;
        // it only checks the shape. (These are shapes, not an app allowlist.)
        assert!(is_safe_launch_arg("-NoStartupMovies"));
        assert!(is_safe_launch_arg("-nologo"));
        assert!(is_safe_launch_arg("-skip_intro"));
        assert!(is_safe_launch_arg("--skip-intro"));
        assert!(is_safe_launch_arg("-intro=0"));
        assert!(is_safe_launch_arg(&format!("-{}", "a".repeat(MAX_LEN - 1))));
    }

    #[test]
    fn anything_the_shell_would_read_as_a_command_is_refused() {
        assert!(!is_safe_launch_arg("-a b")); // whitespace splits into two args
        assert!(!is_safe_launch_arg("-a;b")); // shell metacharacters
        assert!(!is_safe_launch_arg("-a&b"));
        assert!(!is_safe_launch_arg("-a|b"));
        assert!(!is_safe_launch_arg("-a$(b)"));
        assert!(!is_safe_launch_arg("-a%b%"));
        assert!(!is_safe_launch_arg("-a^b"));
        assert!(!is_safe_launch_arg("-a\"b"));
        assert!(!is_safe_launch_arg("-a\nb"));
    }

    #[test]
    fn anything_the_game_would_read_as_a_command_or_a_place_is_refused() {
        assert!(!is_safe_launch_arg("+quit")); // a console line in many engines
        assert!(!is_safe_launch_arg("+com_skipIntroVideo=1"));
        assert!(!is_safe_launch_arg("-ExecCmds=quit")); // a value naming a command
        assert!(!is_safe_launch_arg("-log=../../x")); // a relative path
        assert!(!is_safe_launch_arg("-exec=C:\\evil.exe")); // an absolute path
        assert!(!is_safe_launch_arg("/skipintro")); // a path root on half the world
        assert!(!is_safe_launch_arg("-connect=1.2.3.4:27015")); // an address
        assert!(!is_safe_launch_arg("-a.b")); // a file name
        assert!(!is_safe_launch_arg("-intro=")); // a value that is not there
        assert!(!is_safe_launch_arg("-intro=1=2"));
        assert!(!is_safe_launch_arg("-intro=-1"));
    }

    #[test]
    fn a_proposal_is_trimmed_or_dropped_never_repaired() {
        assert_eq!(clean_launch_argument("  -nologo "), Some("-nologo".into()));
        assert_eq!(clean_launch_argument("-a;calc"), None);
        assert_eq!(clean_launch_argument("-a calc"), None);
        assert_eq!(clean_launch_argument("   "), None);
    }

    #[test]
    fn what_is_not_a_switch_is_refused() {
        assert!(!is_safe_launch_arg("")); // empty
        assert!(!is_safe_launch_arg("nointro")); // not a flag
        assert!(!is_safe_launch_arg("-")); // a dash and nothing
        assert!(!is_safe_launch_arg("--")); // the end-of-options marker
        assert!(!is_safe_launch_arg("---x")); // a third dash
        assert!(!is_safe_launch_arg("-1080p")); // a name must start with a letter
        assert!(!is_safe_launch_arg("-=1"));
        assert!(!is_safe_launch_arg("-nologö")); // ASCII only
        assert!(!is_safe_launch_arg(&format!("-{}", "a".repeat(MAX_LEN)))); // too long
    }
}
