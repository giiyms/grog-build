//! Slash-argument parser for `/advisor`.

use crate::seats::is_effort_token;

/// Parsed `/advisor` verb. Model tokens are resolved later against the
/// live primary model (complement / cycle need it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdvisorVerb {
    /// Bare `/advisor` — session-scoped enable/disable toggle.
    Toggle,
    /// `/advisor on` or `/advisor on luna [xhigh]`.
    On { spec: Option<ModelSpec> },
    Off,
    Status,
    /// `/advisor model` with no extra args — pager opens the shared picker
    /// targeting Advisor; headless shell cycles as a fallback.
    OpenPicker,
    /// `/advisor cycle` — walk luna → opus → sonnet → agy.
    Cycle,
    Dump,
    /// `/advisor luna` / `/advisor sonnet high` — set + enable.
    Set(ModelSpec),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSpec {
    pub raw: String,
    pub effort: Option<String>,
}

pub fn parse_verb(args: &str) -> Result<AdvisorVerb, String> {
    let mut tokens: Vec<&str> = args.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(AdvisorVerb::Toggle);
    }
    let effort = if tokens
        .last()
        .is_some_and(|t| is_effort_token(t) && tokens.len() > 1)
    {
        Some(tokens.pop().unwrap().to_ascii_lowercase())
    } else {
        None
    };
    let first = tokens[0].to_ascii_lowercase();
    match first.as_str() {
        "off" | "disable" | "stop" => Ok(AdvisorVerb::Off),
        "status" | "info" => Ok(AdvisorVerb::Status),
        "dump" => Ok(AdvisorVerb::Dump),
        "model" if tokens.len() == 1 => Ok(AdvisorVerb::OpenPicker),
        "cycle" => Ok(AdvisorVerb::Cycle),
        "on" | "enable" | "start" => {
            let spec = match tokens.get(1) {
                None => None,
                Some(model) => Some(ModelSpec {
                    raw: model.to_string(),
                    effort,
                }),
            };
            Ok(AdvisorVerb::On { spec })
        }
        "model" => Ok(AdvisorVerb::Set(ModelSpec {
            raw: tokens[1].to_string(),
            effort,
        })),
        _ => {
            let raw = tokens.join(" ");
            Ok(AdvisorVerb::Set(ModelSpec { raw, effort }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_on_off_status_dump_cycle() {
        assert_eq!(parse_verb("").unwrap(), AdvisorVerb::Toggle);
        assert_eq!(parse_verb("off").unwrap(), AdvisorVerb::Off);
        assert_eq!(parse_verb("status").unwrap(), AdvisorVerb::Status);
        assert_eq!(parse_verb("dump").unwrap(), AdvisorVerb::Dump);
        assert_eq!(parse_verb("model").unwrap(), AdvisorVerb::OpenPicker);
        assert_eq!(parse_verb("cycle").unwrap(), AdvisorVerb::Cycle);
        assert_eq!(
            parse_verb("on").unwrap(),
            AdvisorVerb::On { spec: None }
        );
    }

    #[test]
    fn on_luna_and_bare_luna_enable() {
        let on = parse_verb("on luna").unwrap();
        assert_eq!(
            on,
            AdvisorVerb::On {
                spec: Some(ModelSpec {
                    raw: "luna".into(),
                    effort: None,
                })
            }
        );
        let set = parse_verb("luna").unwrap();
        assert_eq!(
            set,
            AdvisorVerb::Set(ModelSpec {
                raw: "luna".into(),
                effort: None,
            })
        );
        let opus = parse_verb("on opus").unwrap();
        assert!(matches!(opus, AdvisorVerb::On { spec: Some(s) } if s.raw == "opus"));
        let sonnet = parse_verb("sonnet").unwrap();
        assert!(matches!(sonnet, AdvisorVerb::Set(s) if s.raw == "sonnet"));
    }

    #[test]
    fn trailing_effort() {
        match parse_verb("luna xhigh").unwrap() {
            AdvisorVerb::Set(spec) => {
                assert_eq!(spec.raw, "luna");
                assert_eq!(spec.effort.as_deref(), Some("xhigh"));
            }
            other => panic!("{other:?}"),
        }
        match parse_verb("opus medium").unwrap() {
            AdvisorVerb::Set(spec) => {
                assert_eq!(spec.raw, "opus");
                assert_eq!(spec.effort.as_deref(), Some("medium"));
            }
            other => panic!("{other:?}"),
        }
        match parse_verb("sonnet high").unwrap() {
            AdvisorVerb::Set(spec) => {
                assert_eq!(spec.raw, "sonnet");
                assert_eq!(spec.effort.as_deref(), Some("high"));
            }
            other => panic!("{other:?}"),
        }
        match parse_verb("on luna xhigh").unwrap() {
            AdvisorVerb::On {
                spec: Some(spec),
            } => {
                assert_eq!(spec.raw, "luna");
                assert_eq!(spec.effort.as_deref(), Some("xhigh"));
            }
            other => panic!("{other:?}"),
        }
    }
}
