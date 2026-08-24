//! Fallback catalog when `agy models` is unavailable.
//! Live discovery slugifies `agy models` display names the same way.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AntigravityModel {
    pub id: &'static str,
    pub display_name: &'static str,
}

pub const ANTIGRAVITY_FALLBACK_MODELS: &[AntigravityModel] = &[
    AntigravityModel {
        id: "gemini-3.6-flash",
        display_name: "Gemini 3.6 Flash",
    },
    AntigravityModel {
        id: "gemini-3.6-flash-low",
        display_name: "Gemini 3.6 Flash Low",
    },
    AntigravityModel {
        id: "gemini-3.6-flash-high",
        display_name: "Gemini 3.6 Flash High",
    },
    AntigravityModel {
        id: "gemini-3.1-pro",
        display_name: "Gemini 3.1 Pro",
    },
];

pub fn slugify_model(display: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true;
    for ch in display.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_agy_display_names() {
        assert_eq!(
            slugify_model("Gemini 3.6 Flash (Medium)"),
            "gemini-3-6-flash-medium"
        );
        assert_eq!(slugify_model("Gemini 3.1 Pro"), "gemini-3-1-pro");
    }
}
