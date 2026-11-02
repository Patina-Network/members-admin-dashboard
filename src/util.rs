/// trim `value` if exists, else mapping blank strings to `None`.
pub fn trim_string(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();

    if trimmed.is_empty() {
        None
    } else if trimmed.len() == value.len() {
        Some(value)
    } else {
        Some(trimmed.to_owned())
    }
}

pub fn trim_string_or_clear(value: Option<Option<String>>) -> Option<String> {
    value.map(|value| trim_string(value).unwrap_or_default())
}
