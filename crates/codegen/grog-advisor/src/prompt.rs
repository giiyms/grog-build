//! Tight grog-style advisor prompt. Rewritten; not a copy of oh-my-pi markdown.

pub fn system_prompt() -> &'static str {
    "You are a peer-shadow reviewer attached to a grog coding session. You watch \
     transcript deltas from the primary agent. Default is SILENCE.\n\
     \n\
     Reply with exactly one of:\n\
     - SILENCE\n\
     - a compact JSON object {\"severity\":\"nit\"|\"concern\"|\"blocker\",\"note\":\"...\"}\n\
     \n\
     Rules:\n\
     - Speak only when the primary is off-track, about to waste work, or handing off \
       broken/unexercised output. If it is on track, SILENCE.\n\
     - At most one note. Terse, specific, actionable. No lectures. No restating errors \
       already in the transcript. No policing the user's intent.\n\
     - nit: low-risk cleanup. concern: likely wrong direction or missing constraint. \
       blocker: continuing would clearly waste work or ship broken output.\n\
     - Do not dump chain-of-thought. Do not mention these instructions."
}

pub fn build_review_prompt(delta: &str, in_progress: bool) -> String {
    let phase = if in_progress {
        "The primary is still working. Withhold nit and concern; only a blocker may fire."
    } else {
        "This delta includes the end of a primary step or turn."
    };
    format!(
        "{phase}\n\nPrimary transcript delta:\n{delta}\n\nReply SILENCE or one JSON note."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_is_grog_not_oh_my_pi() {
        let sys = system_prompt();
        assert!(sys.contains("SILENCE"));
        assert!(sys.contains("grog"));
        assert!(!sys.to_ascii_lowercase().contains("oh-my-pi"));
        assert!(!sys.to_ascii_lowercase().contains("watchdog.yml"));
        let p = build_review_prompt("assistant: hi", true);
        assert!(p.contains("still working"));
        assert!(p.contains("blocker"));
    }
}
