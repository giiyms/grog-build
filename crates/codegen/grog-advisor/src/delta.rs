//! Transcript delta rendering. Advisor-injected lines are stripped so the
//! sidecar never reviews its own notes.

/// One primary-transcript item the advisor may see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptItem {
    pub role: &'static str,
    pub text: String,
    pub tool_name: Option<String>,
}

pub const ADVISOR_LINE_PREFIX: &str = "[advisor ";

pub fn is_advisor_injected(text: &str) -> bool {
    let t = text.trim();
    t.contains(ADVISOR_LINE_PREFIX) || t.contains("<grog-advisory")
}

/// Render items after `cursor`, skipping the advisor's own injected notes.
/// Returns `(rendered, new_cursor)`.
pub fn render_delta(items: &[TranscriptItem], cursor: usize) -> (String, usize) {
    let new_cursor = items.len();
    if cursor >= items.len() {
        return (String::new(), new_cursor);
    }
    let mut lines = Vec::new();
    let mut mentions = Vec::new();
    for item in &items[cursor..] {
        if is_advisor_injected(&item.text) {
            continue;
        }
        collect_file_mentions(&item.text, &mut mentions);
        match item.role {
            "assistant" => {
                if let Some(tool) = &item.tool_name {
                    lines.push(format!("assistant tool_call {tool}: {}", clip(&item.text, 800)));
                } else {
                    lines.push(format!("assistant: {}", clip(&item.text, 2000)));
                }
            }
            "tool" => {
                let name = item.tool_name.as_deref().unwrap_or("tool");
                lines.push(format!("tool_result {name}: {}", clip(&item.text, 1200)));
            }
            "user" => lines.push(format!("user: {}", clip(&item.text, 2000))),
            other => {
                if !item.text.trim().is_empty() {
                    lines.push(format!("{other}: {}", clip(&item.text, 800)));
                }
            }
        }
    }
    if !mentions.is_empty() {
        mentions.sort();
        mentions.dedup();
        lines.push(format!(
            "file mentions in this delta (names only, not contents): {}",
            mentions.join(", ")
        ));
    }
    (lines.join("\n"), new_cursor)
}

fn clip(s: &str, max: usize) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.len() <= max {
        return t;
    }
    let mut end = max.saturating_sub(1);
    while end > 0 && !t.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &t[..end])
}

fn collect_file_mentions(text: &str, out: &mut Vec<String>) {
    for token in text.split_whitespace() {
        let t = token.trim_matches(|c: char| {
            matches!(c, ',' | '.' | ';' | ':' | ')' | '(' | '"' | '\'' | '`' | '[' | ']')
        });
        if t.len() < 3 {
            continue;
        }
        let looks_like_path = t.contains('/')
            || t.contains('\\')
            || (t.contains('.')
                && t.rsplit_once('.')
                    .is_some_and(|(_, ext)| (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())));
        if looks_like_path {
            out.push(t.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(role: &'static str, text: &str) -> TranscriptItem {
        TranscriptItem {
            role,
            text: text.into(),
            tool_name: None,
        }
    }

    #[test]
    fn strips_advisor_notes_from_next_delta() {
        let items = vec![
            item("user", "fix the cache"),
            item("assistant", "I'll edit src/cache.rs"),
            item("user", "[advisor opus] concern: tests never ran"),
            item("assistant", "running tests"),
        ];
        let (delta, cursor) = render_delta(&items, 0);
        assert!(!delta.contains("[advisor opus]"));
        assert!(delta.contains("fix the cache"));
        assert!(delta.contains("running tests"));
        assert_eq!(cursor, 4);
        let (again, _) = render_delta(&items, cursor);
        assert!(again.is_empty());
    }

    #[test]
    fn cursor_skips_already_seen() {
        let items = vec![item("user", "a"), item("assistant", "b")];
        let (d, c) = render_delta(&items, 1);
        assert!(!d.contains("user: a"));
        assert!(d.contains("assistant: b"));
        assert_eq!(c, 2);
    }

    #[test]
    fn file_mentions_from_delta() {
        let items = vec![item("assistant", "edit src/lib.rs and crates/foo/bar.rs")];
        let (d, _) = render_delta(&items, 0);
        assert!(d.contains("src/lib.rs"));
        assert!(d.contains("crates/foo/bar.rs"));
    }
}
