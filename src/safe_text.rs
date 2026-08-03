//! Control-character escaping for untrusted text reaching stdout/stderr
//! (TASK-0318).
//!
//! A filename (or an underlying error message that embeds one — e.g.
//! `walkdir`'s `Display` impl, or `trash::Error`'s `Display` impl, which
//! formats its whole enum via `Debug` and so carries any path a variant
//! holds) can contain raw control bytes. Printed verbatim, those bytes let a
//! hostile filename do two distinct things: an ANSI/terminal escape sequence
//! (starting with ESC, `\u{1b}`) can inject cursor moves, screen clears, or
//! worse into the user's terminal; a raw newline can forge an extra line in
//! line-oriented output — the human report (`report.rs`) is exactly that, so
//! a filename containing `\n` could otherwise fabricate a plausible `KEEP`
//! row or a fake `Summary:` line.

use std::borrow::Cow;

/// Escape control characters in `text` so it is safe to print as a single,
/// un-forgeable plain-text line.
///
/// Covers Unicode general category `Cc` only — C0 (0x00-0x1F, including `\n`
/// and ESC), DEL (0x7F), and C1 (0x80-0x9F) — via `char::is_control()`.
/// Every non-control character (including non-Latin scripts, emoji, and
/// combining marks) renders exactly as it was; only control characters are
/// rewritten, using Rust's own escape syntax (`char::escape_default`, e.g.
/// ESC becomes the literal text `\u{1b}`, `\n` becomes `\n`).
///
/// Deliberately does **not** escape category `Cf` (bidi overrides such as
/// U+202E RTLO, zero-width characters): that is filename *spoofing*, a
/// different concern from terminal *control injection*, and escaping it
/// would mangle legitimate right-to-left filenames.
///
/// Returns a borrowed `Cow` when `text` has no control characters (the
/// common case), so escaping is allocation-free unless a control character
/// is actually present.
pub fn escape_control_chars(text: &str) -> Cow<'_, str> {
    if !text.contains(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if c.is_control() {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::escape_control_chars;

    #[test]
    fn control_characters_are_escaped_not_passed_through_raw() {
        let text = "evil\u{1b}[2Jname.jpg";
        let escaped = escape_control_chars(text);

        assert!(
            !escaped.contains('\u{1b}'),
            "the raw ESC byte must not appear in the escaped output; got {escaped:?}"
        );
        assert!(
            escaped.contains("\\u{1b}"),
            "the ESC must render as a visible Rust-style escape; got {escaped:?}"
        );
        assert!(
            escaped.starts_with("evil") && escaped.ends_with("name.jpg"),
            "every non-control character must render exactly as before; got {escaped:?}"
        );
    }

    #[test]
    fn ordinary_text_renders_unchanged() {
        let text = "/normal/photo.jpg";
        assert_eq!(escape_control_chars(text), text);
    }

    #[test]
    fn non_latin_scripts_and_emoji_are_never_escaped() {
        // Only Cc (general category "control") is in scope. Non-Latin
        // scripts, emoji, and combining marks must render byte-for-byte
        // unchanged — this is what would break if is_control() were ever
        // swapped for a broader "non-ASCII" or "non-printable" check.
        let text = "фото-😀-中-e\u{301}.jpg"; // includes a combining acute accent
        assert_eq!(escape_control_chars(text), text);
    }

    #[test]
    fn newline_is_escaped_closing_the_line_forgery_channel() {
        // A raw '\n' in escaped output would let a hostile filename inject
        // an extra line into line-oriented output (report.rs). Escaping it
        // is what closes that, not just the ANSI-injection channel.
        let text = "a\nb";
        let escaped = escape_control_chars(text);
        assert_eq!(escaped, "a\\nb");
        assert_eq!(
            escaped.lines().count(),
            1,
            "the escaped text must render as a single line; got {escaped:?}"
        );
    }
}
