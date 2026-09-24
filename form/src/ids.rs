// Generated file: it is replaced whole on every update, so a change made here is not kept.

//! The shape of a title's identity where it travels as text — in a link, in a
//! report — and the one encoding it travels in. Anything outside these shapes
//! is refused, never repaired.

/// Longest store id accepted (ASCII).
pub const MAX_ID_LEN: usize = 64;
/// Longest executable file name accepted.
pub const MAX_EXE_LEN: usize = 128;

/// A store's own id: letters, digits, `-`, `_` and `.`, nothing that could
/// close a field, start a path or hide a second value.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A file NAME: no separators, no reserved characters, no control bytes.
pub fn valid_exe(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_EXE_LEN
        && s != "."
        && s != ".."
        && !s.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

/// RFC 3986 percent-encoding of everything but the unreserved set.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_a_store_s_own_token_and_nothing_else() {
        assert!(valid_id("1091500"));
        assert!(valid_id("Pub.Title"));
        assert!(valid_id("a-b_c"));
        for bad in ["", "1;drop", "a b", "../x", "c:\\x", "[x](y)", "a\u{202E}b"] {
            assert!(!valid_id(bad), "{bad:?}");
        }
        assert!(valid_id(&"a".repeat(MAX_ID_LEN)));
        assert!(!valid_id(&"a".repeat(MAX_ID_LEN + 1)));
    }

    #[test]
    fn an_executable_is_a_file_name_never_a_path() {
        assert!(valid_exe("Cyberpunk2077.exe"));
        assert!(valid_exe("My Game.exe"));
        for bad in [
            "", ".", "..", "a/b.exe", "a\\b.exe", "c:x.exe", "a\"b.exe", "a|b", "a\nb",
        ] {
            assert!(!valid_exe(bad), "{bad:?}");
        }
        assert!(!valid_exe(&"a".repeat(MAX_EXE_LEN + 1)));
    }

    #[test]
    fn only_the_unreserved_set_travels_as_itself() {
        assert_eq!(percent_encode("steam:42"), "steam%3A42");
        assert_eq!(percent_encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(percent_encode("é"), "%C3%A9");
        assert_eq!(percent_encode("A-z_0.9~"), "A-z_0.9~");
    }
}
