//! Hand-rolled protobuf walker for agy `steps.step_payload` blobs.
//!
//! Field numbers are reverse-engineered (see pi-antigravity-bridge
//! `src/protobuf.ts`). Unknown fields are skipped so newer agy versions
//! keep decoding.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field<'a> {
    pub number: u32,
    pub wire: u8,
    pub bytes: Option<&'a [u8]>,
    pub varint: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub input_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodedStep {
    Text(String),
    Tool(ToolCall),
    Title(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ProtobufError {
    #[error("truncated protobuf")]
    Truncated,
    #[error("varint overflow")]
    VarintOverflow,
    #[error("unsupported wire type {0}")]
    WireType(u8),
}

pub fn walk_fields(buf: &[u8]) -> Result<Vec<Field<'_>>, ProtobufError> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        let (tag, after) = read_varint(buf, i)?;
        i = after;
        let number = (tag >> 3) as u32;
        let wire = (tag & 7) as u8;
        match wire {
            0 => {
                let (val, after) = read_varint(buf, i)?;
                i = after;
                out.push(Field {
                    number,
                    wire,
                    bytes: None,
                    varint: Some(val),
                });
            }
            2 => {
                let (len, after) = read_varint(buf, i)?;
                i = after;
                let len = len as usize;
                if i + len > buf.len() {
                    return Err(ProtobufError::Truncated);
                }
                out.push(Field {
                    number,
                    wire,
                    bytes: Some(&buf[i..i + len]),
                    varint: None,
                });
                i += len;
            }
            1 => {
                if i + 8 > buf.len() {
                    return Err(ProtobufError::Truncated);
                }
                i += 8;
                out.push(Field {
                    number,
                    wire,
                    bytes: None,
                    varint: None,
                });
            }
            5 => {
                if i + 4 > buf.len() {
                    return Err(ProtobufError::Truncated);
                }
                i += 4;
                out.push(Field {
                    number,
                    wire,
                    bytes: None,
                    varint: None,
                });
            }
            other => return Err(ProtobufError::WireType(other)),
        }
    }
    Ok(out)
}

fn read_varint(buf: &[u8], mut i: usize) -> Result<(u64, usize), ProtobufError> {
    let mut result = 0u64;
    for shift in (0..70).step_by(7) {
        if i >= buf.len() {
            return Err(ProtobufError::Truncated);
        }
        let byte = buf[i];
        i += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, i));
        }
        if shift >= 63 {
            return Err(ProtobufError::VarintOverflow);
        }
    }
    Err(ProtobufError::VarintOverflow)
}

fn length_field<'a>(buf: &'a [u8], number: u32) -> Result<Option<&'a [u8]>, ProtobufError> {
    for f in walk_fields(buf)? {
        if f.number == number && f.wire == 2 {
            return Ok(f.bytes);
        }
    }
    Ok(None)
}

pub fn extract_agent_text(payload: &[u8]) -> Result<Option<String>, ProtobufError> {
    let Some(agent) = length_field(payload, 20)? else {
        return Ok(None);
    };
    let Some(text) = length_field(agent, 1)? else {
        return Ok(None);
    };
    Ok(Some(String::from_utf8_lossy(text).into_owned()))
}

pub fn extract_tool_call(payload: &[u8]) -> Result<Option<ToolCall>, ProtobufError> {
    let Some(tool_run) = length_field(payload, 5)? else {
        return Ok(None);
    };
    let Some(tool_call) = length_field(tool_run, 4)? else {
        return Ok(None);
    };
    let mut name = String::new();
    let mut input_json = String::new();
    for f in walk_fields(tool_call)? {
        if f.wire != 2 {
            continue;
        }
        let bytes = f.bytes.unwrap_or(&[]);
        let s = String::from_utf8_lossy(bytes).into_owned();
        match f.number {
            2 => {
                if name.is_empty() {
                    name = s;
                }
            }
            9 if name.is_empty() => name = s,
            3 => input_json = s,
            _ => {}
        }
    }
    if name.is_empty() && input_json.is_empty() {
        return Ok(None);
    }
    Ok(Some(ToolCall { name, input_json }))
}

pub fn extract_title(payload: &[u8]) -> Result<Option<String>, ProtobufError> {
    let Some(title_update) = length_field(payload, 30)? else {
        return Ok(None);
    };
    Ok(length_field(title_update, 4)?.map(|b| String::from_utf8_lossy(b).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
    }

    fn encode_key(field: u32, wire: u8, out: &mut Vec<u8>) {
        encode_varint(u64::from(field) << 3 | u64::from(wire), out);
    }

    fn encode_len(field: u32, payload: &[u8], out: &mut Vec<u8>) {
        encode_key(field, 2, out);
        encode_varint(payload.len() as u64, out);
        out.extend_from_slice(payload);
    }

    #[test]
    fn extracts_agent_text_field_20_1() {
        let mut inner = Vec::new();
        encode_len(1, b"hello from agy", &mut inner);
        let mut payload = Vec::new();
        encode_len(20, &inner, &mut payload);
        encode_len(99, b"ignored", &mut payload);
        assert_eq!(
            extract_agent_text(&payload).unwrap().as_deref(),
            Some("hello from agy")
        );
    }

    #[test]
    fn extracts_tool_call_name_from_field_9_fallback() {
        let mut call = Vec::new();
        encode_len(9, b"run_command", &mut call);
        encode_len(3, br#"{"cmd":"ls"}"#, &mut call);
        let mut run = Vec::new();
        encode_len(4, &call, &mut run);
        let mut payload = Vec::new();
        encode_len(5, &run, &mut payload);
        let tool = extract_tool_call(&payload).unwrap().unwrap();
        assert_eq!(tool.name, "run_command");
        assert_eq!(tool.input_json, r#"{"cmd":"ls"}"#);
    }

    #[test]
    fn extracts_title_field_30_4() {
        let mut inner = Vec::new();
        encode_len(4, b"Fix the parser", &mut inner);
        let mut payload = Vec::new();
        encode_len(30, &inner, &mut payload);
        assert_eq!(
            extract_title(&payload).unwrap().as_deref(),
            Some("Fix the parser")
        );
    }
}
