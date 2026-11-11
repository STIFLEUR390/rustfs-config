//! Scrubbing of credentials before an event or a log leaves the process.
//!
//! Commands handle RustFS access/secret keys and the AWS SDK quotes signed URLs and
//! signature parameters in its error messages, so every report is masked here first
//! (AGENTS.md: no secret in logs or error reports).

/// Keys whose value is masked up to the next separator.
const SENSITIVE_KEYS: &[&str] = &[
    "x-amz-credential",
    "x-amz-signature",
    "x-amz-security-token",
    "authorization",
    "aws_access_key_id",
    "aws_secret_access_key",
    "aws_session_token",
    "access_key",
    "secret_key",
    "accesskey",
    "secretkey",
    "session_token",
    "sessiontoken",
    "password",
];

/// Ends a masked value.
const SEPARATORS: &[char] = &[
    ' ', '\t', '\n', '\r', '"', '\'', ',', '}', ')', ']', '&', ';', '<', '>',
];

const MASK: &str = "***";

/// Masks every value that follows a sensitive key and every AWS-style `AKIA…` key id.
pub fn redact(text: &str) -> String {
    let mut out = text.to_owned();
    for key in SENSITIVE_KEYS {
        out = mask_values_after(&out, key);
    }
    mask_aws_key_ids(&out)
}

fn mask_values_after(text: &str, key: &str) -> String {
    // ASCII-only lowering keeps byte offsets aligned with `text`.
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut from = 0;

    while let Some(offset) = lower[from..].find(key) {
        let hit = from + offset;
        // `access_key` also occurs inside `aws_access_key_id`: only whole names count.
        let glued = text[..hit]
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if glued {
            out.push_str(&text[from..hit + key.len()]);
            from = hit + key.len();
            continue;
        }
        let rest = &text[hit + key.len()..];
        let skip: usize = rest
            .chars()
            .take_while(|c| matches!(c, '=' | ':' | ' ' | '"' | '\''))
            .map(char::len_utf8)
            .sum();
        let value_start = hit + key.len() + skip;
        let value_len: usize = text[value_start..]
            .chars()
            .take_while(|c| !SEPARATORS.contains(c))
            .map(char::len_utf8)
            .sum();

        out.push_str(&text[from..value_start]);
        if value_len > 0 {
            out.push_str(MASK);
        }
        from = value_start + value_len;
    }

    out.push_str(&text[from..]);
    out
}

fn mask_aws_key_ids(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(offset) = rest.find("AKIA") {
        let (head, tail) = rest.split_at(offset);
        out.push_str(head);

        let bytes = tail.as_bytes();
        let mut id_len = 4;
        while id_len < bytes.len()
            && id_len < 20
            && (bytes[id_len].is_ascii_uppercase() || bytes[id_len].is_ascii_digit())
        {
            id_len += 1;
        }

        if id_len == 20 {
            out.push_str("AKIA***");
            rest = &tail[20..];
        } else {
            out.push_str("AKIA");
            rest = &tail[4..];
        }
    }

    out.push_str(rest);
    out
}

/// Masks the message and exception values of an event.
#[cfg(feature = "gui")]
pub fn scrub_event(
    mut event: sentry::protocol::Event<'static>,
) -> sentry::protocol::Event<'static> {
    if let Some(message) = event.message.as_mut() {
        *message = redact(message);
    }
    for exception in event.exception.iter_mut() {
        if let Some(text) = exception.value.as_mut() {
            *text = redact(text);
        }
    }
    event
}

/// Masks the body and the string attributes of a log entry.
#[cfg(feature = "gui")]
pub fn scrub_log(mut log: sentry::protocol::Log) -> sentry::protocol::Log {
    log.body = redact(&log.body);
    for attribute in log.attributes.values_mut() {
        if let sentry::protocol::Value::String(text) = &attribute.0 {
            attribute.0 = sentry::protocol::Value::String(redact(text));
        }
    }
    log
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn masks_query_signature_parameters() {
        let url = "GET https://s3/b?X-Amz-Credential=AKIAIOSFODNN7EXAMPLE%2F20261111&X-Amz-Signature=deadbeef&x=1";
        let masked = redact(url);
        assert!(!masked.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!masked.contains("deadbeef"));
        assert!(masked.contains("x=1"));
        assert!(masked.contains("X-Amz-Credential=***&"));
    }

    #[test]
    fn masks_json_and_ini_style_secrets() {
        assert_eq!(
            redact(r#"{"accessKey": "rc12345", "secretKey": "s3cr3t-value"}"#),
            r#"{"accessKey": "***", "secretKey": "***"}"#
        );
        assert_eq!(
            redact("aws_access_key_id = rc12345\naws_secret_access_key = s3cr3t"),
            "aws_access_key_id = ***\naws_secret_access_key = ***"
        );
    }

    #[test]
    fn masks_authorization_headers_and_bare_key_ids() {
        let header = "Authorization: AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20261111/eu-west-1/s3/aws4_request";
        let masked = redact(header);
        assert!(masked.starts_with("Authorization: ***"));
        assert!(!masked.contains("AKIAIOSFODNN7EXAMPLE"));
        assert_eq!(redact("key AKIAIOSFODNN7EXAMPLE rejected"), "key AKIA*** rejected");
    }

    #[test]
    fn keeps_plain_text_untouched() {
        let text = "bucket demo exists; 3 objects; region eu-west-1";
        assert_eq!(redact(text), text);
    }
}
