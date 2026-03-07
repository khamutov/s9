use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};

use super::RepoError;

/// Encodes a `(updated_at, id)` pair into a base64url cursor string.
pub fn encode_cursor(updated_at: &DateTime<Utc>, id: i64) -> String {
    let plain = format!("{},{id}", updated_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    URL_SAFE_NO_PAD.encode(plain.as_bytes())
}

/// Decodes a base64url cursor string back into `(updated_at, id)`.
pub fn decode_cursor(cursor: &str) -> Result<(DateTime<Utc>, i64), RepoError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| RepoError::Conflict("invalid cursor".to_string()))?;

    let plain =
        String::from_utf8(bytes).map_err(|_| RepoError::Conflict("invalid cursor".to_string()))?;

    let (ts_str, id_str) = plain
        .rsplit_once(',')
        .ok_or_else(|| RepoError::Conflict("invalid cursor".to_string()))?;

    let updated_at: DateTime<Utc> = ts_str
        .parse()
        .map_err(|_| RepoError::Conflict("invalid cursor".to_string()))?;

    let id: i64 = id_str
        .parse()
        .map_err(|_| RepoError::Conflict("invalid cursor".to_string()))?;

    Ok((updated_at, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        let ts = Utc::now();
        let id = 42i64;

        let encoded = encode_cursor(&ts, id);
        let (decoded_ts, decoded_id) = decode_cursor(&encoded).unwrap();

        assert_eq!(decoded_id, id);
        assert_eq!(
            decoded_ts.timestamp_millis(),
            ts.timestamp_millis(),
        );
    }

    #[test]
    fn cursor_invalid() {
        assert!(matches!(
            decode_cursor("not-valid-base64!!!"),
            Err(RepoError::Conflict(_))
        ));
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(b"no-comma")),
            Err(RepoError::Conflict(_))
        ));
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(b"bad-date,42")),
            Err(RepoError::Conflict(_))
        ));
        assert!(matches!(
            decode_cursor(&URL_SAFE_NO_PAD.encode(b"2024-01-01T00:00:00.000Z,notanumber")),
            Err(RepoError::Conflict(_))
        ));
    }
}
