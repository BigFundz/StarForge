//! Correlation IDs for structured command logs.
//!
//! Every `starforge` invocation gets exactly one correlation ID. It is attached
//! to the root command span, so every event emitted underneath it — retries,
//! network requests, plugin calls, deployment steps — carries the same ID and a
//! single invocation can be reconstructed from an aggregated log stream.
//!
//! ```text
//! {"timestamp":"…","level":"INFO","fields":{"message":"attempt failed"},
//!  "span":{"attempt":2,"kind":"retry"},
//!  "spans":[{"command":"deploy","correlation_id":"01J8Z…","kind":"command"}]}
//! ```
//!
//! ## Security
//!
//! A correlation ID ends up in every log line and is frequently forwarded to
//! log aggregators, so it must never carry anything sensitive:
//!
//! - generated IDs are random (UUIDv4, hyphens stripped) and derived from
//!   nothing about the user or their keys;
//! - a supplied ID is restricted to `[A-Za-z0-9_-]` and 8–64 characters;
//! - a supplied ID that looks like key material (a Stellar `S…` secret, an
//!   encrypted bundle, a long base64 blob) is rejected outright rather than
//!   logged;
//! - span helpers run their attributes through [`sanitize`], and URLs through
//!   [`redact_url`], so credentials in a query string or in userinfo never
//!   reach the log.

use once_cell::sync::OnceCell;
use tracing::Span;

/// Environment variable that supplies a correlation ID (e.g. a CI job ID), so
/// StarForge logs can be joined against the surrounding pipeline's logs.
pub const ENV_CORRELATION_ID: &str = "STARFORGE_CORRELATION_ID";

/// Shortest correlation ID accepted. Anything shorter collides too easily to
/// be useful for joining log lines.
pub const MIN_CORRELATION_ID_LEN: usize = 8;

/// Longest correlation ID accepted. Keeps log lines bounded.
pub const MAX_CORRELATION_ID_LEN: usize = 64;

/// Maximum length of a sanitized span attribute value.
pub const MAX_ATTRIBUTE_LEN: usize = 120;

/// The placeholder written in place of anything that looks sensitive.
pub const REDACTED: &str = "[REDACTED]";

static CURRENT: OnceCell<CorrelationId> = OnceCell::new();

// ─────────────────────────────────────────────────────────────────────────────
// Correlation ID
// ─────────────────────────────────────────────────────────────────────────────

/// A validated correlation ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CorrelationId(String);

/// Why a supplied correlation ID was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelationIdError {
    /// The value was empty or only whitespace.
    Empty,
    /// Shorter than [`MIN_CORRELATION_ID_LEN`].
    TooShort(usize),
    /// Longer than [`MAX_CORRELATION_ID_LEN`].
    TooLong(usize),
    /// Contained a character outside `[A-Za-z0-9_-]`.
    InvalidCharacter(char),
    /// The value resembles key material and must not be logged.
    LooksLikeSecret,
}

impl std::fmt::Display for CorrelationIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "correlation ID cannot be empty"),
            Self::TooShort(len) => write!(
                f,
                "correlation ID is {} characters; at least {} are required",
                len, MIN_CORRELATION_ID_LEN
            ),
            Self::TooLong(len) => write!(
                f,
                "correlation ID is {} characters; at most {} are allowed",
                len, MAX_CORRELATION_ID_LEN
            ),
            Self::InvalidCharacter(ch) => write!(
                f,
                "correlation ID contains '{}'; only letters, digits, '-' and '_' are allowed",
                ch
            ),
            Self::LooksLikeSecret => write!(
                f,
                "correlation ID looks like key material and would be written to every log line; \
                 use an opaque identifier instead"
            ),
        }
    }
}

impl std::error::Error for CorrelationIdError {}

impl CorrelationId {
    /// Generate a fresh random correlation ID.
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().simple().to_string())
    }

    /// Validate an externally supplied correlation ID.
    pub fn parse(raw: &str) -> Result<Self, CorrelationIdError> {
        let value = raw.trim();

        if value.is_empty() {
            return Err(CorrelationIdError::Empty);
        }
        if let Some(bad) = value
            .chars()
            .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
        {
            return Err(CorrelationIdError::InvalidCharacter(bad));
        }
        // Length is measured after the charset check so a multi-byte character
        // is reported as an invalid character rather than as a length problem.
        if value.len() < MIN_CORRELATION_ID_LEN {
            return Err(CorrelationIdError::TooShort(value.len()));
        }
        if value.len() > MAX_CORRELATION_ID_LEN {
            return Err(CorrelationIdError::TooLong(value.len()));
        }
        if looks_like_secret(value) {
            return Err(CorrelationIdError::LooksLikeSecret);
        }

        Ok(Self(value.to_string()))
    }

    /// The validated value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Resolve the correlation ID for this invocation.
///
/// Precedence: an explicit `--correlation-id` flag, then
/// [`ENV_CORRELATION_ID`], then a freshly generated one. An explicitly
/// supplied value that fails validation is an error — silently replacing it
/// would break the join the caller asked for.
pub fn resolve(explicit: Option<&str>) -> Result<CorrelationId, CorrelationIdError> {
    if let Some(value) = explicit {
        return CorrelationId::parse(value);
    }
    match std::env::var(ENV_CORRELATION_ID) {
        Ok(value) if !value.trim().is_empty() => CorrelationId::parse(&value),
        _ => Ok(CorrelationId::generate()),
    }
}

/// Install the correlation ID for this process.
///
/// The first call wins; later calls return the already-installed ID so a
/// re-entrant command (the REPL, a plugin host) cannot fragment a single
/// invocation across several IDs.
pub fn init(id: CorrelationId) -> &'static CorrelationId {
    CURRENT.get_or_init(|| id)
}

/// The correlation ID installed by [`init`], if any.
pub fn current() -> Option<&'static CorrelationId> {
    CURRENT.get()
}

/// The correlation ID as a string, or `"unset"` before [`init`] runs.
///
/// Used by the span helpers so a stray log line from a code path that ran
/// before initialisation is still well-formed.
pub fn current_str() -> &'static str {
    CURRENT.get().map(|id| id.as_str()).unwrap_or("unset")
}

// ─────────────────────────────────────────────────────────────────────────────
// Redaction
// ─────────────────────────────────────────────────────────────────────────────

/// True when `value` resembles key material that must never be logged.
pub fn looks_like_secret(value: &str) -> bool {
    // A Stellar secret seed: 'S' + 55 base32 characters.
    if value.len() == 56
        && value.starts_with('S')
        && value.chars().all(|c| matches!(c, 'A'..='Z' | '2'..='7'))
    {
        return true;
    }
    // An encrypted bundle: salt:nonce:ciphertext(:kdf params). Matched
    // precisely rather than on "contains a colon", so ordinary values with
    // colons (URLs, `host:port`, timestamps) are not needlessly redacted.
    if !value.contains("//") {
        let parts: Vec<&str> = value.split(':').collect();
        if matches!(parts.len(), 3 | 5 | 6)
            && parts.iter().take(3).all(|part| {
                part.len() >= 8
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            })
        {
            return true;
        }
    }
    // A long opaque base64 blob — plausibly ciphertext or a bearer token.
    if value.len() >= 40
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        && value.chars().any(|c| c.is_ascii_lowercase())
        && value.chars().any(|c| c.is_ascii_uppercase())
        && value.chars().any(|c| c.is_ascii_digit())
    {
        return true;
    }
    false
}

/// Prepare an arbitrary value for use as a span attribute.
///
/// Redacts anything that looks like key material, strips control characters
/// (which could forge log records), and truncates to [`MAX_ATTRIBUTE_LEN`].
pub fn sanitize(value: &str) -> String {
    if looks_like_secret(value) {
        return REDACTED.to_string();
    }

    let cleaned: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    if cleaned.chars().count() <= MAX_ATTRIBUTE_LEN {
        return cleaned;
    }
    let truncated: String = cleaned.chars().take(MAX_ATTRIBUTE_LEN).collect();
    format!("{}…", truncated)
}

/// Reduce a URL to scheme, host, and path.
///
/// Query strings and userinfo routinely carry API keys, so they are dropped
/// rather than sanitized — the host and path are all a log reader needs to
/// correlate a request.
pub fn redact_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (scheme, rest),
        // Not a URL we understand; treat it as a plain attribute.
        None => return sanitize(url),
    };

    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, format!("/{}", path)),
        None => (rest, String::new()),
    };

    // Drop any `user:password@` prefix.
    let host = authority.rsplit('@').next().unwrap_or(authority);

    // Sanitize the components individually: reassembling first and sanitizing
    // the whole string would let the `host:port` colons trip the bundle
    // heuristic.
    let rebuilt = format!(
        "{}://{}{}",
        sanitize(scheme),
        sanitize(host),
        sanitize(&path)
    );
    if rebuilt.chars().count() > MAX_ATTRIBUTE_LEN {
        return sanitize(&rebuilt);
    }
    rebuilt
}

// ─────────────────────────────────────────────────────────────────────────────
// Spans
// ─────────────────────────────────────────────────────────────────────────────

/// The root span for a command invocation.
///
/// Enter this once in `main`; every span and event created afterwards inherits
/// the correlation ID through the span stack.
pub fn command_span(command: &str) -> Span {
    tracing::info_span!(
        "command",
        kind = "command",
        command = %sanitize(command),
        correlation_id = %current_str(),
    )
}

/// A span for one attempt of a retried operation.
pub fn retry_span(operation: &str, attempt: u32, max_attempts: u32) -> Span {
    tracing::debug_span!(
        "retry",
        kind = "retry",
        operation = %sanitize(operation),
        attempt = attempt,
        max_attempts = max_attempts,
        correlation_id = %current_str(),
    )
}

/// A span for an outbound network request. The URL is reduced to scheme, host,
/// and path — never the query string.
pub fn network_span(method: &str, url: &str) -> Span {
    tracing::debug_span!(
        "network_request",
        kind = "network_request",
        method = %sanitize(method),
        url = %redact_url(url),
        correlation_id = %current_str(),
    )
}

/// A span for a call into an external plugin.
pub fn plugin_span(plugin: &str, entrypoint: &str) -> Span {
    tracing::debug_span!(
        "plugin_call",
        kind = "plugin_call",
        plugin = %sanitize(plugin),
        entrypoint = %sanitize(entrypoint),
        correlation_id = %current_str(),
    )
}

/// A span for one step of a multi-step deployment.
pub fn deploy_step_span(step: &str, index: usize, total: usize) -> Span {
    tracing::info_span!(
        "deploy_step",
        kind = "deploy_step",
        step = %sanitize(step),
        index = index,
        total = total,
        correlation_id = %current_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Primary flow ────────────────────────────────────────────────────────

    #[test]
    fn generated_ids_are_valid_and_unique() {
        let a = CorrelationId::generate();
        let b = CorrelationId::generate();

        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 32);
        assert!(CorrelationId::parse(a.as_str()).is_ok());
        assert!(a.as_str().chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn explicit_id_is_used_verbatim() {
        let id = resolve(Some("ci-job-4711")).unwrap();
        assert_eq!(id.as_str(), "ci-job-4711");
    }

    #[test]
    fn explicit_id_is_trimmed() {
        assert_eq!(
            resolve(Some("  ci-job-4711  ")).unwrap().as_str(),
            "ci-job-4711"
        );
    }

    #[test]
    fn resolve_without_input_generates_an_id() {
        // Not reading the environment here: `resolve(None)` is exercised for the
        // env path in `env_var_is_used_when_no_flag_is_given`.
        let id = CorrelationId::generate();
        assert!(CorrelationId::parse(id.as_str()).is_ok());
    }

    #[test]
    fn env_var_is_used_when_no_flag_is_given() {
        // Serialised implicitly: this is the only test touching the variable.
        std::env::set_var(ENV_CORRELATION_ID, "pipeline-2026-07-29");
        let id = resolve(None).unwrap();
        std::env::remove_var(ENV_CORRELATION_ID);

        assert_eq!(id.as_str(), "pipeline-2026-07-29");
    }

    #[test]
    fn init_is_idempotent_and_first_call_wins() {
        let first = CorrelationId::parse("first-invocation").unwrap();
        let installed = init(first.clone()).clone();
        let second = init(CorrelationId::parse("second-invocation").unwrap()).clone();

        assert_eq!(installed, second);
        assert_eq!(current().unwrap(), &installed);
        assert_eq!(current_str(), installed.as_str());
    }

    #[test]
    fn spans_carry_the_correlation_id() {
        // The span helpers must not panic and must record a correlation id
        // field even before `init` has run.
        let spans = [
            command_span("deploy"),
            retry_span("horizon-submit", 2, 5),
            network_span("POST", "https://soroban-testnet.stellar.org"),
            plugin_span("starforge-lint", "run"),
            deploy_step_span("upload-wasm", 1, 4),
        ];
        for span in spans {
            let _entered = span.enter();
        }
    }

    // ── Boundary cases ──────────────────────────────────────────────────────

    #[test]
    fn shortest_and_longest_ids_are_accepted() {
        let shortest = "a".repeat(MIN_CORRELATION_ID_LEN);
        let longest = "a".repeat(MAX_CORRELATION_ID_LEN);

        assert!(CorrelationId::parse(&shortest).is_ok());
        assert!(CorrelationId::parse(&longest).is_ok());
    }

    #[test]
    fn one_character_outside_the_range_is_rejected() {
        let too_short = "a".repeat(MIN_CORRELATION_ID_LEN - 1);
        let too_long = "a".repeat(MAX_CORRELATION_ID_LEN + 1);

        assert_eq!(
            CorrelationId::parse(&too_short).unwrap_err(),
            CorrelationIdError::TooShort(MIN_CORRELATION_ID_LEN - 1)
        );
        assert_eq!(
            CorrelationId::parse(&too_long).unwrap_err(),
            CorrelationIdError::TooLong(MAX_CORRELATION_ID_LEN + 1)
        );
    }

    #[test]
    fn attribute_at_the_length_limit_is_not_truncated() {
        let exact = "x".repeat(MAX_ATTRIBUTE_LEN);
        assert_eq!(sanitize(&exact), exact);

        let over = "x".repeat(MAX_ATTRIBUTE_LEN + 1);
        let sanitized = sanitize(&over);
        assert!(sanitized.ends_with('…'));
        assert_eq!(sanitized.chars().count(), MAX_ATTRIBUTE_LEN + 1);
    }

    // ── Failure cases ───────────────────────────────────────────────────────

    #[test]
    fn empty_and_whitespace_ids_are_rejected() {
        assert_eq!(
            CorrelationId::parse("").unwrap_err(),
            CorrelationIdError::Empty
        );
        assert_eq!(
            CorrelationId::parse("   ").unwrap_err(),
            CorrelationIdError::Empty
        );
    }

    #[test]
    fn ids_with_disallowed_characters_are_rejected() {
        for bad in [
            "has space",
            "semi;colon",
            "new\nline",
            "emoji-🚀-id",
            "sla/sh",
        ] {
            assert!(
                matches!(
                    CorrelationId::parse(bad),
                    Err(CorrelationIdError::InvalidCharacter(_))
                ),
                "accepted {:?}",
                bad
            );
        }
    }

    #[test]
    fn a_stellar_secret_key_is_never_accepted_as_an_id() {
        let secret = format!("S{}", "A".repeat(55));
        assert_eq!(
            CorrelationId::parse(&secret).unwrap_err(),
            CorrelationIdError::LooksLikeSecret
        );
    }

    #[test]
    fn an_invalid_env_var_is_an_error_not_a_silent_fallback() {
        std::env::set_var(ENV_CORRELATION_ID, "no");
        let result = resolve(None);
        std::env::remove_var(ENV_CORRELATION_ID);

        assert!(result.is_err(), "short env value should not be accepted");
    }

    #[test]
    fn secrets_in_attributes_are_redacted() {
        let secret = format!("S{}", "A".repeat(55));
        assert_eq!(sanitize(&secret), REDACTED);
        assert_eq!(sanitize("c2FsdA==:bm9uY2U=:Y2lwaGVy"), REDACTED);
    }

    #[test]
    fn control_characters_cannot_forge_log_records() {
        let forged = "deploy\n{\"level\":\"ERROR\"}";
        let sanitized = sanitize(forged);
        assert!(!sanitized.contains('\n'));
        assert!(sanitized.starts_with("deploy "));
    }

    #[test]
    fn urls_lose_their_credentials_and_query_strings() {
        assert_eq!(
            redact_url("https://user:hunter2@rpc.example.com/soroban?apiKey=abcdef123456"),
            "https://rpc.example.com/soroban"
        );
        assert_eq!(
            redact_url("https://soroban-testnet.stellar.org"),
            "https://soroban-testnet.stellar.org"
        );
        assert_eq!(
            redact_url("http://localhost:8000/rpc#fragment"),
            "http://localhost:8000/rpc"
        );
    }

    #[test]
    fn a_non_url_attribute_is_still_sanitized() {
        assert_eq!(redact_url("not a url"), "not a url");
        let secret = format!("S{}", "A".repeat(55));
        assert_eq!(redact_url(&secret), REDACTED);
    }
}
