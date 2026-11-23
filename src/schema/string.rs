//! String schema builder, string format rules, and sanitization.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
};

#[cfg(feature = "regex")]
use regex::Regex;
use serde_json::Value;
#[cfg(feature = "async")]
use {crate::rule::AsyncCustomRule, futures_util::future::BoxFuture};

use crate::{
    coercion::trim_whitespace,
    context::ValidationContext,
    error::{RuleParams, ValidationError, ValidationIssue},
    messages::MessagesProvider,
    rule::{CustomRule, RuleContext},
    schema::Schema,
};

const VALID_EMAIL_LOCAL: [bool; 256] = {
    let mut table = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        if (b >= b'a' && b <= b'z')
            || (b >= b'A' && b <= b'Z')
            || (b >= b'0' && b <= b'9')
            || b == b'.'
            || b == b'!'
            || b == b'#'
            || b == b'$'
            || b == b'%'
            || b == b'&'
            || b == b'\''
            || b == b'*'
            || b == b'+'
            || b == b'/'
            || b == b'='
            || b == b'?'
            || b == b'^'
            || b == b'_'
            || b == b'`'
            || b == b'{'
            || b == b'|'
            || b == b'}'
            || b == b'~'
            || b == b'-'
        {
            table[i] = true;
        }
        i += 1;
    }
    table
};

const VALID_DOMAIN_CHAR: [bool; 256] = {
    let mut table = [false; 256];
    let mut i = 0;
    while i < 256 {
        let b = i as u8;
        if (b >= b'a' && b <= b'z')
            || (b >= b'A' && b <= b'Z')
            || (b >= b'0' && b <= b'9')
            || b == b'-'
        {
            table[i] = true;
        }
        i += 1;
    }
    table
};

#[inline]
pub(crate) fn is_valid_email_fast(s: &str) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if !(3..=254).contains(&len) {
        return false;
    }

    let Some(at_pos) = bytes.iter().position(|&b| b == b'@') else {
        return false;
    };

    if at_pos == 0 || at_pos > 64 || at_pos >= len - 2 {
        return false;
    }

    let local = &bytes[..at_pos];
    let domain = &bytes[at_pos + 1..];

    if domain.len() > 253 {
        return false;
    }

    for &b in local {
        if !VALID_EMAIL_LOCAL[b as usize] {
            return false;
        }
    }

    let mut has_dot = false;
    let mut label_len = 0;
    let mut last_byte = 0u8;

    for &b in domain {
        if b == b'.' {
            if label_len == 0 || label_len > 63 || !last_byte.is_ascii_alphanumeric() {
                return false;
            }
            has_dot = true;
            label_len = 0;
        } else if label_len == 0 {
            if !b.is_ascii_alphanumeric() {
                return false;
            }
            label_len = 1;
            last_byte = b;
        } else if !VALID_DOMAIN_CHAR[b as usize] {
            return false;
        } else {
            label_len += 1;
            last_byte = b;
        }
    }

    has_dot && label_len > 0 && label_len <= 63 && last_byte.is_ascii_alphanumeric()
}

#[inline]
pub(crate) fn is_valid_uuid_fast(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    if !matches!(bytes[14], b'1'..=b'8') {
        return false;
    }
    if !matches!(bytes[19], b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
        return false;
    }

    #[inline(always)]
    fn is_hex_slice(slice: &[u8]) -> bool {
        slice.iter().all(|b| b.is_ascii_hexdigit())
    }

    is_hex_slice(&bytes[0..8])
        && is_hex_slice(&bytes[9..13])
        && is_hex_slice(&bytes[15..18])
        && is_hex_slice(&bytes[20..23])
        && is_hex_slice(&bytes[24..36])
}

#[inline]
pub(crate) fn is_valid_ipv4_fast(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 7 || bytes.len() > 15 {
        return false;
    }
    let mut octets = 0;
    let mut current: u32 = 0;
    let mut has_digits = false;
    let mut leading_zero = false;

    for &b in bytes {
        if b == b'.' {
            if !has_digits || current > 255 {
                return false;
            }
            octets += 1;
            if octets > 3 {
                return false;
            }
            current = 0;
            has_digits = false;
            leading_zero = false;
        } else if b.is_ascii_digit() {
            if leading_zero {
                return false;
            }
            if !has_digits && b == b'0' {
                leading_zero = true;
            }
            has_digits = true;
            current = current * 10 + (b - b'0') as u32;
            if current > 255 {
                return false;
            }
        } else {
            return false;
        }
    }
    octets == 3 && has_digits && current <= 255
}

#[inline]
pub(crate) fn is_valid_url_fast(s: &str) -> bool {
    let bytes = s.as_bytes();
    let rest = if bytes.starts_with(b"https://") {
        &bytes[8..]
    } else if bytes.starts_with(b"http://") {
        &bytes[7..]
    } else if bytes.starts_with(b"ftp://") {
        &bytes[6..]
    } else {
        return false;
    };

    if rest.len() < 2 {
        return false;
    }

    let first = rest[0];
    if first.is_ascii_whitespace() || matches!(first, b'/' | b'$' | b'.' | b'?' | b'#') {
        return false;
    }

    for &b in &rest[1..] {
        if b.is_ascii_whitespace() {
            return false;
        }
    }
    true
}

#[derive(Clone)]
enum StringPredicate {
    MinLen(usize),
    MaxLen(usize),
    ExactLen(usize),
    Email,
    Url,
    Uuid,
    Ip,
    Ipv4,
    Ipv6,
    Date,
    Time,
    DateTime,
    CreditCard,
    Ulid,
    HexColor,
    Slug,
    SemVer,
    Base64,
    JsonString,
    InList(Vec<String>),
    NotInList(Vec<String>),
    E164,
    Jwt,
    MacAddress,
    MimeType,
    Base64Url,
    Sha256,
    Sha512,
    Md5,
    NonControl,
    #[cfg(feature = "regex")]
    RegexPattern(Regex),
    Alpha,
    Alphanumeric,
    Ascii,
    StartsWith(String),
    EndsWith(String),
    Contains(String),
    Hostname {
        rfc_1123: bool,
    },
    Domain,
    Fqdn,
    Port,
    Cidr,
    DataUri,
    Latitude,
    Longitude,
    Coordinates,
    PostalCode(String),
    Iban,
    Bic,
    Isbn,
    Isbn10,
    Isbn13,
    Isin,
    Cuid,
    Cuid2,
    Nanoid {
        min_len: usize,
        max_len: usize,
    },
    CountryCode,
    CurrencyCode,
    Hex,
    Digits,
    MinWords(usize),
    MaxWords(usize),
    Words(usize, usize),
    DoesNotContain(String),
    SafeInteger,
    #[cfg(feature = "schema-color")]
    Rgb,
    #[cfg(feature = "schema-color")]
    Rgba,
    #[cfg(feature = "schema-color")]
    Hsl,
    #[cfg(feature = "schema-color")]
    Hsla,
    #[cfg(feature = "schema-crypto")]
    EthereumAddress,
    #[cfg(feature = "schema-crypto")]
    BitcoinAddress,
    #[cfg(feature = "schema-business")]
    EuVat,
    #[cfg(feature = "schema-business")]
    Ein,
    #[cfg(feature = "schema-business")]
    Ssn,
    #[cfg(feature = "schema-identifiers")]
    MongoDbObjectId,
    #[cfg(feature = "schema-identifiers")]
    Bcp47,
    #[cfg(feature = "schema-identifiers")]
    Cron,
    #[cfg(feature = "schema-identifiers")]
    Ksuid,
    #[cfg(feature = "schema-identifiers")]
    Issn,
    #[cfg(feature = "schema-identifiers")]
    Imei,
    #[cfg(feature = "schema-date")]
    Weekend,
    #[cfg(feature = "schema-date")]
    Weekday,
    #[cfg(feature = "schema-date")]
    Duration,
    Custom(Arc<dyn CustomRule<String>>),
}

/// Fluent schema builder for validating and sanitizing string values.
#[derive(Clone)]
pub struct StringSchema {
    predicates: Vec<StringPredicate>,
    should_trim: bool,
    trim_start: bool,
    trim_end: bool,
    empty_as_null: bool,
    to_lower: bool,
    to_upper: bool,
    escape_html: bool,
    normalize_email: bool,
    bail_on_fail: bool,
    inline_message: Option<String>,
    #[cfg(feature = "async")]
    async_rules: Vec<Arc<dyn AsyncCustomRule<String>>>,
}

impl Default for StringSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl StringSchema {
    /// Creates a new empty `StringSchema`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            predicates: Vec::new(),
            should_trim: true,
            trim_start: false,
            trim_end: false,
            empty_as_null: false,
            to_lower: false,
            to_upper: false,
            escape_html: false,
            normalize_email: false,
            bail_on_fail: false,
            inline_message: None,
            #[cfg(feature = "async")]
            async_rules: Vec::new(),
        }
    }

    /// Configures whether leading and trailing whitespace should be trimmed (default: `true`).
    #[must_use]
    pub fn trim(mut self, enabled: bool) -> Self {
        self.should_trim = enabled;
        self
    }

    /// Disables automatic whitespace trimming.
    #[must_use]
    pub fn without_trim(mut self) -> Self {
        self.should_trim = false;
        self
    }

    /// Configures whether empty strings should be treated as null (default: `false`).
    #[must_use]
    pub fn empty_string_as_null(mut self, enabled: bool) -> Self {
        self.empty_as_null = enabled;
        self
    }

    /// Treats empty strings `""` as null.
    ///
    /// Alias for `empty_string_as_null(true)`.
    #[must_use]
    pub fn empty_to_null(self) -> Self {
        self.empty_string_as_null(true)
    }

    /// Converts the validated string to lowercase.
    #[must_use]
    pub fn to_lowercase(mut self) -> Self {
        self.to_lower = true;
        self.to_upper = false;
        self
    }

    /// Converts the validated string to uppercase.
    #[must_use]
    pub fn to_uppercase(mut self) -> Self {
        self.to_upper = true;
        self.to_lower = false;
        self
    }

    /// Configures whether rule evaluation on this field should stop on the first failure.
    #[must_use]
    pub fn bail(mut self, enabled: bool) -> Self {
        self.bail_on_fail = enabled;
        self
    }

    /// Sets an inline custom error message override for this string field.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.inline_message = Some(message.into());
        self
    }

    /// Enforces a minimum character length constraint.
    #[must_use]
    pub fn min_len(mut self, min: usize) -> Self {
        self.predicates.push(StringPredicate::MinLen(min));
        self
    }

    /// Enforces a maximum character length constraint.
    #[must_use]
    pub fn max_len(mut self, max: usize) -> Self {
        self.predicates.push(StringPredicate::MaxLen(max));
        self
    }

    /// Enforces an exact character length constraint.
    #[must_use]
    pub fn exact_len(mut self, length: usize) -> Self {
        self.predicates.push(StringPredicate::ExactLen(length));
        self
    }

    /// Validates that the string matches RFC 5322 email syntax.
    #[must_use]
    pub fn email(mut self) -> Self {
        self.predicates.push(StringPredicate::Email);
        self
    }

    /// Validates that the string matches RFC 3986 URL syntax.
    #[must_use]
    pub fn url(mut self) -> Self {
        self.predicates.push(StringPredicate::Url);
        self
    }

    /// Validates that the string is a valid UUID format (RFC 4122 / RFC 9562).
    #[must_use]
    pub fn uuid(mut self) -> Self {
        self.predicates.push(StringPredicate::Uuid);
        self
    }

    /// Validates that the string is either an IPv4 or IPv6 address.
    #[must_use]
    pub fn ip(mut self) -> Self {
        self.predicates.push(StringPredicate::Ip);
        self
    }

    /// Validates that the string is an IPv4 address.
    #[must_use]
    pub fn ipv4(mut self) -> Self {
        self.predicates.push(StringPredicate::Ipv4);
        self
    }

    /// Validates that the string is an IPv6 address.
    #[must_use]
    pub fn ipv6(mut self) -> Self {
        self.predicates.push(StringPredicate::Ipv6);
        self
    }

    /// Validates that the string matches an ISO 8601 / RFC 3339 calendar date (`YYYY-MM-DD`).
    #[must_use]
    pub fn date(mut self) -> Self {
        self.predicates.push(StringPredicate::Date);
        self
    }

    /// Validates that the string matches an ISO 8601 / RFC 3339 clock time (`HH:MM:SS[...]`).
    #[must_use]
    pub fn time(mut self) -> Self {
        self.predicates.push(StringPredicate::Time);
        self
    }

    /// Validates that the string matches a complete RFC 3339 date-time.
    #[must_use]
    pub fn datetime(mut self) -> Self {
        self.predicates.push(StringPredicate::DateTime);
        self
    }

    /// Validates that the string passes the Luhn checksum formula (ISO/IEC 7812).
    #[must_use]
    pub fn credit_card(mut self) -> Self {
        self.predicates.push(StringPredicate::CreditCard);
        self
    }

    /// Validates that the string is a valid 26-character Crockford Base32 ULID.
    #[must_use]
    pub fn ulid(mut self) -> Self {
        self.predicates.push(StringPredicate::Ulid);
        self
    }

    /// Validates that the string is a valid 3, 4, 6, or 8 character hex color with optional `#`.
    #[must_use]
    pub fn hex_color(mut self) -> Self {
        self.predicates.push(StringPredicate::HexColor);
        self
    }

    /// Validates that the string is a URL-friendly slug (lowercase alphanumeric with single hyphens).
    #[must_use]
    pub fn slug(mut self) -> Self {
        self.predicates.push(StringPredicate::Slug);
        self
    }

    /// Validates that the string conforms to Semantic Versioning 2.0.0.
    #[must_use]
    pub fn semver(mut self) -> Self {
        self.predicates.push(StringPredicate::SemVer);
        self
    }

    /// Validates that the string is valid RFC 4648 Base64 data with proper padding.
    #[must_use]
    pub fn base64(mut self) -> Self {
        self.predicates.push(StringPredicate::Base64);
        self
    }

    /// Validates that the string is a valid JSON payload.
    #[must_use]
    pub fn json_string(mut self) -> Self {
        self.predicates.push(StringPredicate::JsonString);
        self
    }

    /// Validates that the string is contained within the allowed values list.
    #[must_use]
    pub fn in_list(mut self, allowed: &[&str]) -> Self {
        let items = allowed.iter().map(|s| (*s).to_string()).collect();
        self.predicates.push(StringPredicate::InList(items));
        self
    }

    /// Validates that the string is not contained within the disallowed values list.
    #[must_use]
    pub fn not_in_list(mut self, disallowed: &[&str]) -> Self {
        let items = disallowed.iter().map(|s| (*s).to_string()).collect();
        self.predicates.push(StringPredicate::NotInList(items));
        self
    }

    /// Validates that the string matches the ITU-T E.164 international phone number format.
    #[must_use]
    pub fn e164(mut self) -> Self {
        self.predicates.push(StringPredicate::E164);
        self
    }

    /// Validates that the string matches the ITU-T E.164 international phone number format.
    ///
    /// Alias for [`StringSchema::e164`].
    #[must_use]
    pub fn phone_number(self) -> Self {
        self.e164()
    }

    /// Validates that the string is a structurally valid 3-part JSON Web Token (RFC 7519).
    #[must_use]
    pub fn jwt(mut self) -> Self {
        self.predicates.push(StringPredicate::Jwt);
        self
    }

    /// Validates that the string matches an IEEE 802 standard MAC-48 or EUI-64 address.
    #[must_use]
    pub fn mac_address(mut self) -> Self {
        self.predicates.push(StringPredicate::MacAddress);
        self
    }

    /// Validates that the string conforms to RFC 6838 MIME media type syntax.
    #[must_use]
    pub fn mime_type(mut self) -> Self {
        self.predicates.push(StringPredicate::MimeType);
        self
    }

    /// Validates that the string is valid RFC 4648 URL-safe Base64 data.
    #[must_use]
    pub fn base64_url(mut self) -> Self {
        self.predicates.push(StringPredicate::Base64Url);
        self
    }

    /// Validates that the string is a 64-character hexadecimal SHA-256 digest.
    #[must_use]
    pub fn sha256(mut self) -> Self {
        self.predicates.push(StringPredicate::Sha256);
        self
    }

    /// Validates that the string is a 128-character hexadecimal SHA-512 digest.
    #[must_use]
    pub fn sha512(mut self) -> Self {
        self.predicates.push(StringPredicate::Sha512);
        self
    }

    /// Validates that the string is a 32-character hexadecimal MD5 digest.
    #[must_use]
    pub fn md5(mut self) -> Self {
        self.predicates.push(StringPredicate::Md5);
        self
    }

    /// Asserts that the string contains no ASCII control characters (0x00..=0x1F, 0x7F).
    #[must_use]
    pub fn non_control(mut self) -> Self {
        self.predicates.push(StringPredicate::NonControl);
        self
    }

    #[cfg(feature = "regex")]
    /// Enforces matching against a custom regular expression string.
    #[must_use]
    pub fn regex(mut self, pattern: &str) -> Self {
        if let Ok(compiled) = Regex::new(pattern) {
            self.predicates
                .push(StringPredicate::RegexPattern(compiled));
        }
        self
    }

    /// Validates that the string contains only alphabetic characters.
    #[must_use]
    pub fn alpha(mut self) -> Self {
        self.predicates.push(StringPredicate::Alpha);
        self
    }

    /// Validates that the string contains only alphanumeric characters.
    #[must_use]
    pub fn alphanumeric(mut self) -> Self {
        self.predicates.push(StringPredicate::Alphanumeric);
        self
    }

    /// Validates that the string contains only ASCII characters.
    #[must_use]
    pub fn ascii(mut self) -> Self {
        self.predicates.push(StringPredicate::Ascii);
        self
    }

    /// Enforces that the string starts with the given prefix.
    #[must_use]
    pub fn starts_with(mut self, prefix: impl Into<String>) -> Self {
        self.predicates
            .push(StringPredicate::StartsWith(prefix.into()));
        self
    }

    /// Enforces that the string ends with the given suffix.
    #[must_use]
    pub fn ends_with(mut self, suffix: impl Into<String>) -> Self {
        self.predicates
            .push(StringPredicate::EndsWith(suffix.into()));
        self
    }

    /// Enforces that the string contains the specified substring.
    #[must_use]
    pub fn contains(mut self, expected: impl Into<String>) -> Self {
        self.predicates
            .push(StringPredicate::Contains(expected.into()));
        self
    }

    /// Enforces that the string does not contain the specified substring.
    #[must_use]
    pub fn does_not_contain(mut self, disallowed: impl Into<String>) -> Self {
        self.predicates
            .push(StringPredicate::DoesNotContain(disallowed.into()));
        self
    }

    /// Validates that the string is a valid RFC 1123 hostname.
    #[must_use]
    pub fn hostname(mut self) -> Self {
        self.predicates
            .push(StringPredicate::Hostname { rfc_1123: true });
        self
    }

    /// Validates that the string is a valid RFC 952 hostname (must start with a letter).
    #[must_use]
    pub fn hostname_rfc952(mut self) -> Self {
        self.predicates
            .push(StringPredicate::Hostname { rfc_1123: false });
        self
    }

    /// Validates that the string is a valid domain name with a recognized TLD.
    #[must_use]
    pub fn domain(mut self) -> Self {
        self.predicates.push(StringPredicate::Domain);
        self
    }

    /// Validates that the string is a valid Fully Qualified Domain Name (FQDN).
    #[must_use]
    pub fn fqdn(mut self) -> Self {
        self.predicates.push(StringPredicate::Fqdn);
        self
    }

    /// Validates that the string represents a valid network port (1-65535).
    #[must_use]
    pub fn port(mut self) -> Self {
        self.predicates.push(StringPredicate::Port);
        self
    }

    /// Validates that the string is a valid CIDR notation for IPv4 or IPv6.
    #[must_use]
    pub fn cidr(mut self) -> Self {
        self.predicates.push(StringPredicate::Cidr);
        self
    }

    /// Validates that the string is a valid RFC 2397 Data URI.
    #[must_use]
    pub fn data_uri(mut self) -> Self {
        self.predicates.push(StringPredicate::DataUri);
        self
    }

    /// Validates that the string parses as a valid geographic latitude (-90.0 to 90.0).
    #[must_use]
    pub fn latitude(mut self) -> Self {
        self.predicates.push(StringPredicate::Latitude);
        self
    }

    /// Validates that the string parses as a valid geographic longitude (-180.0 to 180.0).
    #[must_use]
    pub fn longitude(mut self) -> Self {
        self.predicates.push(StringPredicate::Longitude);
        self
    }

    /// Validates that the string represents valid comma-separated geographic coordinates ("lat, lng").
    #[must_use]
    pub fn coordinates(mut self) -> Self {
        self.predicates.push(StringPredicate::Coordinates);
        self
    }

    /// Validates that the string is a valid postal code for the specified country (e.g. "US", "CA", "GB").
    #[must_use]
    pub fn postal_code(mut self, country: impl Into<String>) -> Self {
        self.predicates
            .push(StringPredicate::PostalCode(country.into()));
        self
    }

    /// Validates that the string is a valid US ZIP or ZIP+4 code.
    #[must_use]
    pub fn zip_code(self) -> Self {
        self.postal_code("US")
    }

    /// Validates that the string is a valid ISO 13616 International Bank Account Number (IBAN).
    #[must_use]
    pub fn iban(mut self) -> Self {
        self.predicates.push(StringPredicate::Iban);
        self
    }

    /// Validates that the string is a valid ISO 9362 Bank Identifier Code (BIC/SWIFT).
    #[must_use]
    pub fn bic(mut self) -> Self {
        self.predicates.push(StringPredicate::Bic);
        self
    }

    /// Validates that the string is a valid ISO 9362 Bank Identifier Code (BIC/SWIFT).
    ///
    /// Alias for [`StringSchema::bic`].
    #[must_use]
    pub fn swift(self) -> Self {
        self.bic()
    }

    /// Validates that the string is a valid ISBN (either ISBN-10 or ISBN-13).
    #[must_use]
    pub fn isbn(mut self) -> Self {
        self.predicates.push(StringPredicate::Isbn);
        self
    }

    /// Validates that the string is a valid 10-digit ISBN.
    #[must_use]
    pub fn isbn10(mut self) -> Self {
        self.predicates.push(StringPredicate::Isbn10);
        self
    }

    /// Validates that the string is a valid 13-digit ISBN.
    #[must_use]
    pub fn isbn13(mut self) -> Self {
        self.predicates.push(StringPredicate::Isbn13);
        self
    }

    /// Validates that the string is a valid ISO 6166 International Securities Identification Number (ISIN).
    #[must_use]
    pub fn isin(mut self) -> Self {
        self.predicates.push(StringPredicate::Isin);
        self
    }

    /// Validates that the string is a valid collision-resistant unique identifier (CUID).
    #[must_use]
    pub fn cuid(mut self) -> Self {
        self.predicates.push(StringPredicate::Cuid);
        self
    }

    /// Validates that the string is a valid CUID2.
    #[must_use]
    pub fn cuid2(mut self) -> Self {
        self.predicates.push(StringPredicate::Cuid2);
        self
    }

    /// Validates that the string is a valid NanoID with default length 21.
    #[must_use]
    pub fn nanoid(self) -> Self {
        self.nanoid_len(21)
    }

    /// Validates that the string is a valid NanoID with the specified length.
    #[must_use]
    pub fn nanoid_len(mut self, len: usize) -> Self {
        self.predicates.push(StringPredicate::Nanoid {
            min_len: len,
            max_len: len,
        });
        self
    }

    /// Validates that the string is a valid ISO 3166-1 alpha-2 or alpha-3 country code.
    #[must_use]
    pub fn country_code(mut self) -> Self {
        self.predicates.push(StringPredicate::CountryCode);
        self
    }

    /// Validates that the string is a valid ISO 4217 three-letter currency code.
    #[must_use]
    pub fn currency_code(mut self) -> Self {
        self.predicates.push(StringPredicate::CurrencyCode);
        self
    }

    /// Validates that the string contains only hexadecimal characters.
    #[must_use]
    pub fn hex(mut self) -> Self {
        self.predicates.push(StringPredicate::Hex);
        self
    }

    /// Validates that the string contains only numeric digits.
    #[must_use]
    pub fn digits(mut self) -> Self {
        self.predicates.push(StringPredicate::Digits);
        self
    }

    /// Enforces that the string has at least `min` whitespace-separated words.
    #[must_use]
    pub fn min_words(mut self, min: usize) -> Self {
        self.predicates.push(StringPredicate::MinWords(min));
        self
    }

    /// Enforces that the string has at most `max` whitespace-separated words.
    #[must_use]
    pub fn max_words(mut self, max: usize) -> Self {
        self.predicates.push(StringPredicate::MaxWords(max));
        self
    }

    /// Enforces that the string has between `min` and `max` whitespace-separated words.
    #[must_use]
    pub fn words(mut self, min: usize, max: usize) -> Self {
        self.predicates.push(StringPredicate::Words(min, max));
        self
    }

    /// Enforces that the string parses as an IEEE 754 safe integer in [-(2^53 - 1), 2^53 - 1].
    #[must_use]
    pub fn safe_integer(mut self) -> Self {
        self.predicates.push(StringPredicate::SafeInteger);
        self
    }

    #[cfg(feature = "schema-color")]
    /// Validates that the string matches the CSS RGB color format.
    #[must_use]
    pub fn rgb(mut self) -> Self {
        self.predicates.push(StringPredicate::Rgb);
        self
    }

    #[cfg(feature = "schema-color")]
    /// Validates that the string matches the CSS RGBA color format.
    #[must_use]
    pub fn rgba(mut self) -> Self {
        self.predicates.push(StringPredicate::Rgba);
        self
    }

    #[cfg(feature = "schema-color")]
    /// Validates that the string matches the CSS HSL color format.
    #[must_use]
    pub fn hsl(mut self) -> Self {
        self.predicates.push(StringPredicate::Hsl);
        self
    }

    #[cfg(feature = "schema-color")]
    /// Validates that the string matches the CSS HSLA color format.
    #[must_use]
    pub fn hsla(mut self) -> Self {
        self.predicates.push(StringPredicate::Hsla);
        self
    }

    #[cfg(feature = "schema-crypto")]
    /// Validates that the string is a 42-character hex Ethereum address.
    #[must_use]
    pub fn ethereum_address(mut self) -> Self {
        self.predicates.push(StringPredicate::EthereumAddress);
        self
    }

    #[cfg(feature = "schema-crypto")]
    /// Validates that the string is a valid Bitcoin address (Base58Check or Bech32).
    #[must_use]
    pub fn bitcoin_address(mut self) -> Self {
        self.predicates.push(StringPredicate::BitcoinAddress);
        self
    }

    #[cfg(feature = "schema-business")]
    /// Validates that the string is a valid European Union VAT number.
    #[must_use]
    pub fn eu_vat(mut self) -> Self {
        self.predicates.push(StringPredicate::EuVat);
        self
    }

    #[cfg(feature = "schema-business")]
    /// Validates that the string matches the US Employer Identification Number (EIN) format.
    #[must_use]
    pub fn ein(mut self) -> Self {
        self.predicates.push(StringPredicate::Ein);
        self
    }

    #[cfg(feature = "schema-business")]
    /// Validates that the string matches the US Social Security Number (SSN) format.
    #[must_use]
    pub fn ssn(mut self) -> Self {
        self.predicates.push(StringPredicate::Ssn);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is a 24-character hexadecimal MongoDB ObjectId.
    #[must_use]
    pub fn mongodb_object_id(mut self) -> Self {
        self.predicates.push(StringPredicate::MongoDbObjectId);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is an IETF BCP 47 language tag.
    #[must_use]
    pub fn bcp47(mut self) -> Self {
        self.predicates.push(StringPredicate::Bcp47);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is a valid cron schedule expression.
    #[must_use]
    pub fn cron(mut self) -> Self {
        self.predicates.push(StringPredicate::Cron);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is a 27-character base62 KSUID.
    #[must_use]
    pub fn ksuid(mut self) -> Self {
        self.predicates.push(StringPredicate::Ksuid);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is an 8-character International Standard Serial Number (ISSN).
    #[must_use]
    pub fn issn(mut self) -> Self {
        self.predicates.push(StringPredicate::Issn);
        self
    }

    #[cfg(feature = "schema-identifiers")]
    /// Validates that the string is a 15-digit International Mobile Equipment Identity (IMEI).
    #[must_use]
    pub fn imei(mut self) -> Self {
        self.predicates.push(StringPredicate::Imei);
        self
    }

    #[cfg(feature = "schema-date")]
    /// Enforces that the date string falls on a weekend (Saturday or Sunday).
    #[must_use]
    pub fn weekend(mut self) -> Self {
        self.predicates.push(StringPredicate::Weekend);
        self
    }

    #[cfg(feature = "schema-date")]
    /// Enforces that the date string falls on a weekday (Monday through Friday).
    #[must_use]
    pub fn weekday(mut self) -> Self {
        self.predicates.push(StringPredicate::Weekday);
        self
    }

    #[cfg(feature = "schema-date")]
    /// Validates that the string adheres to the ISO 8601 Duration format.
    #[must_use]
    pub fn duration(mut self) -> Self {
        self.predicates.push(StringPredicate::Duration);
        self
    }

    /// Trims leading whitespace from the string.
    #[must_use]
    pub fn trim_start(mut self) -> Self {
        self.should_trim = false;
        self.trim_start = true;
        self
    }

    /// Trims trailing whitespace from the string.
    #[must_use]
    pub fn trim_end(mut self) -> Self {
        self.should_trim = false;
        self.trim_end = true;
        self
    }

    /// Escapes HTML special characters in the string (`&`, `<`, `>`, `"`, `'`).
    #[must_use]
    pub fn escape_html(mut self) -> Self {
        self.escape_html = true;
        self
    }

    /// Normalizes email addresses by trimming whitespace and lowercasing the domain component.
    #[must_use]
    pub fn normalize_email(mut self) -> Self {
        self.normalize_email = true;
        self
    }

    /// Attaches a custom synchronous validation rule.
    #[must_use]
    pub fn rule<R: CustomRule<String> + 'static>(mut self, rule: R) -> Self {
        self.predicates
            .push(StringPredicate::Custom(Arc::new(rule)));
        self
    }

    #[cfg(feature = "async")]
    /// Attaches an asynchronous custom validation rule.
    #[must_use]
    pub fn rule_async<R: AsyncCustomRule<String> + 'static>(mut self, rule: R) -> Self {
        self.async_rules.push(Arc::new(rule));
        self
    }

    #[cfg(feature = "async")]
    /// Refines the string value with an asynchronous predicate closure.
    #[must_use]
    pub fn refine_async<F>(mut self, predicate: F) -> Self
    where
        F: (for<'a> Fn(&'a str) -> BoxFuture<'a, Result<(), String>>) + Send + Sync + 'static,
    {
        let inline = self.inline_message.clone();
        let predicate = Arc::new(predicate);

        struct StringRefineRule<F> {
            predicate: Arc<F>,
            inline: Option<String>,
        }

        impl<F> AsyncCustomRule<String> for StringRefineRule<F>
        where
            F: (for<'a> Fn(&'a str) -> BoxFuture<'a, Result<(), String>>) + Send + Sync + 'static,
        {
            fn validate<'a>(
                &'a self,
                value: &'a String,
                ctx: &'a RuleContext<'a>,
            ) -> BoxFuture<'a, Result<(), ValidationIssue>> {
                Box::pin(async move {
                    match (self.predicate)(value.as_str()).await {
                        Ok(()) => Ok(()),
                        Err(err_msg) => {
                            let msg = self.inline.clone().unwrap_or(err_msg);
                            Err(ValidationIssue::new(ctx.path(), "refine_async", msg))
                        }
                    }
                })
            }
        }

        self.async_rules
            .push(Arc::new(StringRefineRule { predicate, inline }));
        self
    }

    fn evaluate_predicate(
        &self,
        predicate: &StringPredicate,
        value: &str,
        rule_context: &RuleContext<'_>,
    ) -> Option<ValidationIssue> {
        let inline = self.inline_message.as_deref();

        match predicate {
            StringPredicate::MinLen(min) => {
                let bytes_len = value.len();
                if bytes_len >= min.saturating_mul(4) {
                    None
                } else {
                    let char_count = if bytes_len < *min || value.is_ascii() {
                        bytes_len
                    } else {
                        value.chars().count()
                    };
                    if char_count < *min {
                        let mut params = RuleParams::new();
                        params.insert("min", min.to_string());
                        params.insert("actual", char_count.to_string());
                        Some(rule_context.create_issue("min_len", params, inline))
                    } else {
                        None
                    }
                }
            }
            StringPredicate::MaxLen(max) => {
                let bytes_len = value.len();
                if bytes_len <= *max {
                    None
                } else {
                    let char_count = if bytes_len > max.saturating_mul(4) || value.is_ascii() {
                        bytes_len
                    } else {
                        value.chars().count()
                    };
                    if char_count > *max {
                        let mut params = RuleParams::new();
                        params.insert("max", max.to_string());
                        params.insert("actual", char_count.to_string());
                        Some(rule_context.create_issue("max_len", params, inline))
                    } else {
                        None
                    }
                }
            }
            StringPredicate::ExactLen(expected) => {
                let bytes_len = value.len();
                let char_count = if bytes_len < *expected
                    || bytes_len > expected.saturating_mul(4)
                    || value.is_ascii()
                {
                    bytes_len
                } else {
                    value.chars().count()
                };
                if char_count != *expected {
                    let mut params = RuleParams::new();
                    params.insert("length", expected.to_string());
                    params.insert("actual", char_count.to_string());
                    Some(rule_context.create_issue("exact_len", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::Email => {
                let is_valid = is_valid_email_fast(value);

                if !is_valid {
                    Some(rule_context.create_issue("email", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Url => {
                let is_valid = is_valid_url_fast(value);

                if !is_valid {
                    Some(rule_context.create_issue("url", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Uuid => {
                let is_valid = is_valid_uuid_fast(value);

                if !is_valid {
                    Some(rule_context.create_issue("uuid", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Ip => {
                if IpAddr::from_str(value).is_err() {
                    Some(rule_context.create_issue("ip", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Ipv4 => {
                if Ipv4Addr::from_str(value).is_err() {
                    Some(rule_context.create_issue("ipv4", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Ipv6 => {
                if Ipv6Addr::from_str(value).is_err() {
                    Some(rule_context.create_issue("ipv6", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Date => {
                if !crate::schema::date::is_valid_rfc3339_date(value) {
                    Some(rule_context.create_issue("date", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Time => {
                if !crate::schema::date::is_valid_rfc3339_time(value) {
                    Some(rule_context.create_issue("time", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::DateTime => {
                if !crate::schema::date::is_valid_rfc3339_datetime(value) {
                    Some(rule_context.create_issue("datetime", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::CreditCard => {
                if !is_valid_credit_card(value) {
                    Some(rule_context.create_issue("credit_card", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Ulid => {
                if !is_valid_ulid(value) {
                    Some(rule_context.create_issue("ulid", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::HexColor => {
                if !is_valid_hex_color(value) {
                    Some(rule_context.create_issue("hex_color", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Slug => {
                if !is_valid_slug(value) {
                    Some(rule_context.create_issue("slug", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::SemVer => {
                if !is_valid_semver(value) {
                    Some(rule_context.create_issue("semver", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Base64 => {
                if !is_valid_base64(value) {
                    Some(rule_context.create_issue("base64", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::JsonString => {
                if serde_json::from_str::<Value>(value).is_err() {
                    Some(rule_context.create_issue("json_string", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::InList(allowed) => {
                if !allowed.iter().any(|item| item == value) {
                    let mut params = RuleParams::new();
                    params.insert("allowed", allowed.join(", "));
                    Some(rule_context.create_issue("in_list", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::NotInList(disallowed) => {
                if disallowed.iter().any(|item| item == value) {
                    let mut params = RuleParams::new();
                    params.insert("disallowed", disallowed.join(", "));
                    Some(rule_context.create_issue("not_in_list", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::E164 => {
                if !is_valid_e164(value) {
                    Some(rule_context.create_issue("e164", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Jwt => {
                if !is_valid_jwt(value) {
                    Some(rule_context.create_issue("jwt", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::MacAddress => {
                if !is_valid_mac_address(value) {
                    Some(rule_context.create_issue("mac_address", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::MimeType => {
                if !is_valid_mime_type(value) {
                    Some(rule_context.create_issue("mime_type", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Base64Url => {
                if !is_valid_base64_url(value) {
                    Some(rule_context.create_issue("base64_url", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Sha256 => {
                if !is_valid_hex_digest(value, 64) {
                    Some(rule_context.create_issue("sha256", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Sha512 => {
                if !is_valid_hex_digest(value, 128) {
                    Some(rule_context.create_issue("sha512", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Md5 => {
                if !is_valid_hex_digest(value, 32) {
                    Some(rule_context.create_issue("md5", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::NonControl => {
                if !has_no_control_characters(value) {
                    Some(rule_context.create_issue("non_control", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "regex")]
            StringPredicate::RegexPattern(regex) => {
                if !regex.is_match(value) {
                    Some(rule_context.create_issue("regex", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Alpha => {
                let is_alpha = !value.is_empty() && value.chars().all(char::is_alphabetic);
                if !is_alpha {
                    Some(rule_context.create_issue("alpha", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Alphanumeric => {
                let is_alnum = !value.is_empty() && value.chars().all(char::is_alphanumeric);
                if !is_alnum {
                    Some(rule_context.create_issue("alphanumeric", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Ascii => {
                if !value.is_ascii() {
                    Some(rule_context.create_issue("ascii", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::StartsWith(prefix) => {
                if !value.starts_with(prefix) {
                    let mut params = RuleParams::new();
                    params.insert("prefix", prefix);
                    Some(rule_context.create_issue("starts_with", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::EndsWith(suffix) => {
                if !value.ends_with(suffix) {
                    let mut params = RuleParams::new();
                    params.insert("suffix", suffix);
                    Some(rule_context.create_issue("ends_with", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::Contains(expected) => {
                if !value.contains(expected) {
                    let mut params = RuleParams::new();
                    params.insert("expected", expected);
                    Some(rule_context.create_issue("contains", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::DoesNotContain(disallowed) => {
                if value.contains(disallowed) {
                    let mut params = RuleParams::new();
                    params.insert("disallowed", disallowed);
                    Some(rule_context.create_issue("does_not_contain", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::Hostname { rfc_1123 } => {
                if !is_valid_hostname(value, *rfc_1123) {
                    Some(rule_context.create_issue("hostname", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Domain => {
                if !is_valid_domain(value) {
                    Some(rule_context.create_issue("domain", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Fqdn => {
                if !is_valid_fqdn(value) {
                    Some(rule_context.create_issue("fqdn", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Port => {
                if !is_valid_port_str(value) {
                    Some(rule_context.create_issue("port", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Cidr => {
                if !is_valid_cidr(value) {
                    Some(rule_context.create_issue("cidr", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::DataUri => {
                if !is_valid_data_uri(value) {
                    Some(rule_context.create_issue("data_uri", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Latitude => {
                if !is_valid_latitude_str(value) {
                    Some(rule_context.create_issue("latitude", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Longitude => {
                if !is_valid_longitude_str(value) {
                    Some(rule_context.create_issue("longitude", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Coordinates => {
                if !is_valid_coordinates_str(value) {
                    Some(rule_context.create_issue("coordinates", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::PostalCode(country) => {
                if !is_valid_postal_code(value, country) {
                    let mut params = RuleParams::new();
                    params.insert("country", country);
                    Some(rule_context.create_issue("postal_code", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::Iban => {
                if !is_valid_iban(value) {
                    Some(rule_context.create_issue("iban", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Bic => {
                if !is_valid_bic(value) {
                    Some(rule_context.create_issue("bic", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Isbn => {
                if !is_valid_isbn(value) {
                    Some(rule_context.create_issue("isbn", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Isbn10 => {
                if !is_valid_isbn10(value) {
                    Some(rule_context.create_issue("isbn10", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Isbn13 => {
                if !is_valid_isbn13(value) {
                    Some(rule_context.create_issue("isbn13", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Isin => {
                if !is_valid_isin(value) {
                    Some(rule_context.create_issue("isin", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Cuid => {
                if !is_valid_cuid(value) {
                    Some(rule_context.create_issue("cuid", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Cuid2 => {
                if !is_valid_cuid2(value) {
                    Some(rule_context.create_issue("cuid2", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Nanoid { min_len, max_len } => {
                if !is_valid_nanoid(value, *min_len, *max_len) {
                    Some(rule_context.create_issue("nanoid", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::CountryCode => {
                if !is_valid_country_code(value) {
                    Some(rule_context.create_issue("country_code", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::CurrencyCode => {
                if !is_valid_currency_code(value) {
                    Some(rule_context.create_issue("currency_code", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Hex => {
                if !is_valid_hex(value) {
                    Some(rule_context.create_issue("hex", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Digits => {
                if !is_valid_digits(value) {
                    Some(rule_context.create_issue("digits", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::MinWords(min) => {
                let count = count_words(value);
                if count < *min {
                    let mut params = RuleParams::new();
                    params.insert("min", min.to_string());
                    params.insert("actual", count.to_string());
                    Some(rule_context.create_issue("min_words", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::MaxWords(max) => {
                let count = count_words(value);
                if count > *max {
                    let mut params = RuleParams::new();
                    params.insert("max", max.to_string());
                    params.insert("actual", count.to_string());
                    Some(rule_context.create_issue("max_words", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::Words(min, max) => {
                let count = count_words(value);
                if count < *min || count > *max {
                    let mut params = RuleParams::new();
                    params.insert("min", min.to_string());
                    params.insert("max", max.to_string());
                    params.insert("actual", count.to_string());
                    Some(rule_context.create_issue("words", params, inline))
                } else {
                    None
                }
            }
            StringPredicate::SafeInteger => {
                const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
                match value.parse::<f64>() {
                    Ok(val)
                        if val.fract() == 0.0
                            && (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&val) =>
                    {
                        None
                    }
                    _ => Some(rule_context.create_issue("safe_integer", RuleParams::new(), inline)),
                }
            }
            #[cfg(feature = "schema-color")]
            StringPredicate::Rgb => {
                if !is_valid_rgb(value, false, true) {
                    Some(rule_context.create_issue("rgb", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-color")]
            StringPredicate::Rgba => {
                if !is_valid_rgb(value, true, true) {
                    Some(rule_context.create_issue("rgba", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-color")]
            StringPredicate::Hsl => {
                if !is_valid_hsl(value, false, true) {
                    Some(rule_context.create_issue("hsl", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-color")]
            StringPredicate::Hsla => {
                if !is_valid_hsl(value, true, true) {
                    Some(rule_context.create_issue("hsla", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-crypto")]
            StringPredicate::EthereumAddress => {
                if !is_valid_ethereum_address(value) {
                    Some(rule_context.create_issue("ethereum_address", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-crypto")]
            StringPredicate::BitcoinAddress => {
                if !is_valid_bitcoin_address(value) {
                    Some(rule_context.create_issue("bitcoin_address", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-business")]
            StringPredicate::EuVat => {
                if !is_valid_eu_vat(value) {
                    Some(rule_context.create_issue("eu_vat", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-business")]
            StringPredicate::Ein => {
                if !is_valid_ein(value) {
                    Some(rule_context.create_issue("ein", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-business")]
            StringPredicate::Ssn => {
                if !is_valid_ssn(value) {
                    Some(rule_context.create_issue("ssn", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::MongoDbObjectId => {
                if !is_valid_mongodb_object_id(value) {
                    Some(rule_context.create_issue("mongodb_object_id", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::Bcp47 => {
                if !is_valid_bcp47(value) {
                    Some(rule_context.create_issue("bcp47", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::Cron => {
                if !is_valid_cron(value) {
                    Some(rule_context.create_issue("cron", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::Ksuid => {
                if !is_valid_ksuid(value) {
                    Some(rule_context.create_issue("ksuid", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::Issn => {
                if !is_valid_issn(value) {
                    Some(rule_context.create_issue("issn", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-identifiers")]
            StringPredicate::Imei => {
                if !is_valid_imei(value) {
                    Some(rule_context.create_issue("imei", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-date")]
            StringPredicate::Weekend => {
                if !crate::schema::date::is_weekend_date(value) {
                    Some(rule_context.create_issue("weekend", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-date")]
            StringPredicate::Weekday => {
                if !crate::schema::date::is_weekday_date(value) {
                    Some(rule_context.create_issue("weekday", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            #[cfg(feature = "schema-date")]
            StringPredicate::Duration => {
                if !crate::schema::date::is_valid_iso8601_duration(value) {
                    Some(rule_context.create_issue("duration", RuleParams::new(), inline))
                } else {
                    None
                }
            }
            StringPredicate::Custom(rule) => {
                let owned = value.to_string();
                rule.validate(&owned, rule_context).err()
            }
        }
    }

    #[inline]
    fn matches_predicate(&self, predicate: &StringPredicate, value: &str) -> bool {
        match predicate {
            StringPredicate::MinLen(expected) => {
                let bytes_len = value.len();
                if bytes_len < *expected {
                    false
                } else if bytes_len >= expected.saturating_mul(4) || value.is_ascii() {
                    true
                } else {
                    value.chars().take(*expected).count() == *expected
                }
            }
            StringPredicate::MaxLen(expected) => {
                let bytes_len = value.len();
                if bytes_len <= *expected {
                    true
                } else if bytes_len > expected.saturating_mul(4) || value.is_ascii() {
                    false
                } else {
                    value.chars().take(expected + 1).count() <= *expected
                }
            }
            StringPredicate::ExactLen(expected) => {
                let bytes_len = value.len();
                if bytes_len < *expected || bytes_len > expected.saturating_mul(4) {
                    false
                } else if bytes_len == *expected && value.is_ascii() {
                    true
                } else {
                    value.chars().take(expected + 1).count() == *expected
                }
            }
            StringPredicate::Email => is_valid_email_fast(value),
            StringPredicate::Url => is_valid_url_fast(value),
            StringPredicate::Uuid => is_valid_uuid_fast(value),
            StringPredicate::Ip => is_valid_ipv4_fast(value) || Ipv6Addr::from_str(value).is_ok(),
            StringPredicate::Ipv4 => is_valid_ipv4_fast(value),
            StringPredicate::Ipv6 => Ipv6Addr::from_str(value).is_ok(),
            StringPredicate::Date => crate::schema::date::is_valid_rfc3339_date(value),
            StringPredicate::Time => crate::schema::date::is_valid_rfc3339_time(value),
            StringPredicate::DateTime => crate::schema::date::is_valid_rfc3339_datetime(value),
            StringPredicate::CreditCard => is_valid_credit_card(value),
            StringPredicate::Ulid => is_valid_ulid(value),
            StringPredicate::HexColor => is_valid_hex_color(value),
            StringPredicate::Slug => is_valid_slug(value),
            StringPredicate::SemVer => is_valid_semver(value),
            StringPredicate::Base64 => is_valid_base64(value),
            StringPredicate::StartsWith(prefix) => value.starts_with(prefix),
            StringPredicate::EndsWith(suffix) => value.ends_with(suffix),
            StringPredicate::Contains(sub) => value.contains(sub),
            StringPredicate::Alpha => {
                if value.is_empty() {
                    false
                } else if value.is_ascii() {
                    value.bytes().all(|b| b.is_ascii_alphabetic())
                } else {
                    value.chars().all(char::is_alphabetic)
                }
            }
            StringPredicate::Alphanumeric => {
                if value.is_empty() {
                    false
                } else if value.is_ascii() {
                    value.bytes().all(|b| b.is_ascii_alphanumeric())
                } else {
                    value.chars().all(char::is_alphanumeric)
                }
            }
            StringPredicate::Ascii => value.is_ascii(),
            _ => {
                let context = ValidationContext::new();
                let rule_ctx = RuleContext::new(
                    "",
                    &context,
                    None,
                    crate::messages::default_messages_provider(),
                );
                self.evaluate_predicate(predicate, value, &rule_ctx)
                    .is_none()
            }
        }
    }

    #[inline]
    fn has_transform(&self) -> bool {
        self.to_lower
            || self.to_upper
            || self.normalize_email
            || self.escape_html
            || self.should_trim
            || self.trim_start
            || self.trim_end
    }

    /// Checks that a string slice conforms to all validation rules without requiring `Value` wrappers or heap allocations.
    #[inline]
    pub fn check_str(&self, input: &str) -> Result<(), ValidationError> {
        if !self.has_transform()
            && (!self.empty_as_null || !input.is_empty())
            && self
                .predicates
                .iter()
                .all(|predicate| self.matches_predicate(predicate, input))
        {
            return Ok(());
        }

        let context = ValidationContext::new();
        let rule_ctx = RuleContext::new(
            "",
            &context,
            None,
            crate::messages::default_messages_provider(),
        );

        let has_transform = self.has_transform();
        let processed: std::borrow::Cow<'_, str> = if has_transform {
            let mut text = input.to_string();
            if self.should_trim {
                text = trim_whitespace(&text).to_string();
            } else {
                if self.trim_start {
                    text = text.trim_start().to_string();
                }
                if self.trim_end {
                    text = text.trim_end().to_string();
                }
            }

            if self.to_lower {
                text = text.to_lowercase();
            } else if self.to_upper {
                text = text.to_uppercase();
            }

            if self.normalize_email {
                text = normalize_email_str(&text);
            }

            if self.escape_html {
                text = escape_html_str(&text);
            }
            std::borrow::Cow::Owned(text)
        } else {
            std::borrow::Cow::Borrowed(input)
        };

        if self.empty_as_null && processed.is_empty() {
            let issue = rule_ctx.create_issue(
                "required",
                RuleParams::new(),
                self.inline_message.as_deref(),
            );
            return Err(ValidationError::single(
                issue.field,
                issue.rule,
                issue.message,
            ));
        }

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, &processed, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates and sanitizes a string slice, returning the processed result.
    pub fn validate_str(&self, input: &str) -> Result<String, ValidationError> {
        let context = ValidationContext::new();
        let rule_ctx = RuleContext::new(
            "",
            &context,
            None,
            crate::messages::default_messages_provider(),
        );

        let has_transform = self.has_transform();
        let processed: std::borrow::Cow<'_, str> = if has_transform {
            let mut text = input.to_string();
            if self.should_trim {
                text = trim_whitespace(&text).to_string();
            } else {
                if self.trim_start {
                    text = text.trim_start().to_string();
                }
                if self.trim_end {
                    text = text.trim_end().to_string();
                }
            }

            if self.to_lower {
                text = text.to_lowercase();
            } else if self.to_upper {
                text = text.to_uppercase();
            }

            if self.normalize_email {
                text = normalize_email_str(&text);
            }

            if self.escape_html {
                text = escape_html_str(&text);
            }
            std::borrow::Cow::Owned(text)
        } else {
            std::borrow::Cow::Borrowed(input)
        };

        if self.empty_as_null && processed.is_empty() {
            let issue = rule_ctx.create_issue(
                "required",
                RuleParams::new(),
                self.inline_message.as_deref(),
            );
            return Err(ValidationError::single(
                issue.field,
                issue.rule,
                issue.message,
            ));
        }

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, &processed, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(processed.into_owned())
        } else {
            Err(errors)
        }
    }
}

impl Schema<String> for StringSchema {
    fn validate_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<String, ValidationError> {
        let rule_ctx = RuleContext::new(path, context, parent_data, messages);

        let s = match value {
            Value::String(s) => s.as_str(),
            Value::Null => {
                let issue = rule_ctx.create_issue(
                    "required",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let issue = rule_ctx.create_issue(
                    "string",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        let has_transform = self.has_transform();
        let processed: std::borrow::Cow<'_, str> = if has_transform {
            let mut text = s.to_string();
            if self.should_trim {
                text = trim_whitespace(&text).to_string();
            } else {
                if self.trim_start {
                    text = text.trim_start().to_string();
                }
                if self.trim_end {
                    text = text.trim_end().to_string();
                }
            }

            if self.to_lower {
                text = text.to_lowercase();
            } else if self.to_upper {
                text = text.to_uppercase();
            }

            if self.normalize_email {
                text = normalize_email_str(&text);
            }

            if self.escape_html {
                text = escape_html_str(&text);
            }
            std::borrow::Cow::Owned(text)
        } else {
            std::borrow::Cow::Borrowed(s)
        };

        if self.empty_as_null && processed.is_empty() {
            let issue = rule_ctx.create_issue(
                "required",
                RuleParams::new(),
                self.inline_message.as_deref(),
            );
            return Err(ValidationError::single(
                issue.field,
                issue.rule,
                issue.message,
            ));
        }

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, &processed, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(processed.into_owned())
        } else {
            Err(errors)
        }
    }

    fn check_path(
        &self,
        value: &Value,
        path: &str,
        context: &ValidationContext,
        parent_data: Option<&Value>,
        messages: &dyn MessagesProvider,
    ) -> Result<(), ValidationError> {
        let s = match value {
            Value::String(s) => s.as_str(),
            Value::Null => {
                let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                let issue = rule_ctx.create_issue(
                    "required",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
            _ => {
                let rule_ctx = RuleContext::new(path, context, parent_data, messages);
                let issue = rule_ctx.create_issue(
                    "string",
                    RuleParams::new(),
                    self.inline_message.as_deref(),
                );
                return Err(ValidationError::single(
                    issue.field,
                    issue.rule,
                    issue.message,
                ));
            }
        };

        if !self.has_transform()
            && (!self.empty_as_null || !s.is_empty())
            && self
                .predicates
                .iter()
                .all(|predicate| self.matches_predicate(predicate, s))
        {
            return Ok(());
        }

        let has_transform = self.has_transform();

        let rule_ctx = RuleContext::new(path, context, parent_data, messages);

        let processed: std::borrow::Cow<'_, str> = if has_transform {
            let mut text = s.to_string();
            if self.should_trim {
                text = trim_whitespace(&text).to_string();
            } else {
                if self.trim_start {
                    text = text.trim_start().to_string();
                }
                if self.trim_end {
                    text = text.trim_end().to_string();
                }
            }

            if self.to_lower {
                text = text.to_lowercase();
            } else if self.to_upper {
                text = text.to_uppercase();
            }

            if self.normalize_email {
                text = normalize_email_str(&text);
            }

            if self.escape_html {
                text = escape_html_str(&text);
            }
            std::borrow::Cow::Owned(text)
        } else {
            std::borrow::Cow::Borrowed(s)
        };

        if self.empty_as_null && processed.is_empty() {
            let issue = rule_ctx.create_issue(
                "required",
                RuleParams::new(),
                self.inline_message.as_deref(),
            );
            return Err(ValidationError::single(
                issue.field,
                issue.rule,
                issue.message,
            ));
        }

        let mut errors = ValidationError::new();
        for predicate in &self.predicates {
            if let Some(issue) = self.evaluate_predicate(predicate, &processed, &rule_ctx) {
                errors.push(issue);
                if self.bail_on_fail {
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[cfg(feature = "async")]
    fn validate_async_path<'a>(
        &'a self,
        value: &'a Value,
        path: &'a str,
        context: &'a ValidationContext,
        parent_data: Option<&'a Value>,
        messages: &'a dyn MessagesProvider,
    ) -> BoxFuture<'a, Result<String, ValidationError>> {
        Box::pin(async move {
            let sync_result = self.validate_path(value, path, context, parent_data, messages);
            let valid_string = match sync_result {
                Ok(val) => val,
                Err(sync_err) => return Err(sync_err),
            };

            let rule_ctx = RuleContext::new(path, context, parent_data, messages);
            let mut errors = ValidationError::new();

            for async_rule in &self.async_rules {
                match async_rule.validate(&valid_string, &rule_ctx).await {
                    Ok(()) => {}
                    Err(issue) => {
                        errors.push(issue);
                        if self.bail_on_fail {
                            break;
                        }
                    }
                }
            }

            if errors.is_empty() {
                Ok(valid_string)
            } else {
                Err(errors)
            }
        })
    }

    fn to_json_schema_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("type".to_string(), Value::String("string".to_string()));
        for pred in &self.predicates {
            match pred {
                StringPredicate::MinLen(min) => {
                    map.insert("minLength".to_string(), Value::Number((*min).into()));
                }
                StringPredicate::MaxLen(max) => {
                    map.insert("maxLength".to_string(), Value::Number((*max).into()));
                }
                StringPredicate::ExactLen(exact) => {
                    map.insert("minLength".to_string(), Value::Number((*exact).into()));
                    map.insert("maxLength".to_string(), Value::Number((*exact).into()));
                }
                StringPredicate::Email => {
                    map.insert("format".to_string(), Value::String("email".to_string()));
                }
                StringPredicate::Url => {
                    map.insert("format".to_string(), Value::String("uri".to_string()));
                }
                StringPredicate::Uuid => {
                    map.insert("format".to_string(), Value::String("uuid".to_string()));
                }
                StringPredicate::Ipv4 => {
                    map.insert("format".to_string(), Value::String("ipv4".to_string()));
                }
                StringPredicate::Ipv6 => {
                    map.insert("format".to_string(), Value::String("ipv6".to_string()));
                }
                StringPredicate::Ip => {
                    map.insert("format".to_string(), Value::String("ip".to_string()));
                }
                StringPredicate::Date => {
                    map.insert("format".to_string(), Value::String("date".to_string()));
                }
                StringPredicate::Time => {
                    map.insert("format".to_string(), Value::String("time".to_string()));
                }
                StringPredicate::DateTime => {
                    map.insert("format".to_string(), Value::String("date-time".to_string()));
                }
                StringPredicate::E164 => {
                    map.insert("format".to_string(), Value::String("tel".to_string()));
                }
                #[cfg(feature = "regex")]
                StringPredicate::RegexPattern(re) => {
                    map.insert(
                        "pattern".to_string(),
                        Value::String(re.as_str().to_string()),
                    );
                }
                StringPredicate::InList(allowed) => {
                    let items = allowed.iter().map(|s| Value::String(s.clone())).collect();
                    map.insert("enum".to_string(), Value::Array(items));
                }
                StringPredicate::Hostname { .. }
                | StringPredicate::Domain
                | StringPredicate::Fqdn => {
                    map.insert("format".to_string(), Value::String("hostname".to_string()));
                }
                StringPredicate::DataUri => {
                    map.insert("format".to_string(), Value::String("uri".to_string()));
                }
                StringPredicate::Hex => {
                    map.insert(
                        "pattern".to_string(),
                        Value::String("^(0[xX])?[0-9a-fA-F]+$".to_string()),
                    );
                }
                StringPredicate::Digits => {
                    map.insert("pattern".to_string(), Value::String("^[0-9]+$".to_string()));
                }
                #[cfg(feature = "schema-identifiers")]
                StringPredicate::MongoDbObjectId => {
                    map.insert(
                        "pattern".to_string(),
                        Value::String("^[0-9a-fA-F]{24}$".to_string()),
                    );
                }
                #[cfg(feature = "schema-crypto")]
                StringPredicate::EthereumAddress => {
                    map.insert(
                        "pattern".to_string(),
                        Value::String("^0[xX][0-9a-fA-F]{40}$".to_string()),
                    );
                }
                #[cfg(feature = "schema-date")]
                StringPredicate::Duration => {
                    map.insert("format".to_string(), Value::String("duration".to_string()));
                }
                _ => {}
            }
        }
        Value::Object(map)
    }
}

fn is_valid_credit_card(s: &str) -> bool {
    let mut digits = [0u8; 32];
    let mut count = 0;
    for b in s.bytes() {
        if b.is_ascii_digit() {
            if count >= 32 {
                return false;
            }
            digits[count] = b - b'0';
            count += 1;
        } else if b != b'-' && b != b' ' {
            return false;
        }
    }
    if !(8..=19).contains(&count) {
        return false;
    }
    let mut sum: u32 = 0;
    let mut alternate = false;
    for &d in digits[..count].iter().rev() {
        let mut n = d as u32;
        if alternate {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alternate = !alternate;
    }
    sum % 10 == 0
}

fn is_valid_ulid(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 26 {
        return false;
    }
    if !matches!(bytes[0], b'0'..=b'7') {
        return false;
    }
    for &b in &bytes[1..] {
        let is_crockford = matches!(
            b,
            b'0'..=b'9'
                | b'A'..=b'H'
                | b'J'..=b'K'
                | b'M'..=b'N'
                | b'P'..=b'T'
                | b'V'..=b'Z'
                | b'a'..=b'h'
                | b'j'..=b'k'
                | b'm'..=b'n'
                | b'p'..=b't'
                | b'v'..=b'z'
        );
        if !is_crockford {
            return false;
        }
    }
    true
}

fn is_valid_hex_color(s: &str) -> bool {
    let bytes = s.strip_prefix('#').unwrap_or(s).as_bytes();
    match bytes.len() {
        3 | 4 | 6 | 8 => bytes.iter().all(u8::is_ascii_hexdigit),
        _ => false,
    }
}

fn is_valid_slug(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes[0] == b'-' || bytes[bytes.len() - 1] == b'-' {
        return false;
    }
    let mut prev_hyphen = false;
    for &b in bytes {
        if b.is_ascii_lowercase() || b.is_ascii_digit() {
            prev_hyphen = false;
        } else if b == b'-' {
            if prev_hyphen {
                return false;
            }
            prev_hyphen = true;
        } else {
            return false;
        }
    }
    true
}

fn is_valid_semver(s: &str) -> bool {
    if s.is_empty() || s.starts_with('v') || s.starts_with('+') || s.starts_with('-') {
        return false;
    }
    let (version_pre, build) = match s.split_once('+') {
        Some((v, b)) => (v, Some(b)),
        None => (s, None),
    };
    if let Some(b) = build {
        if b.is_empty() {
            return false;
        }
        for ident in b.split('.') {
            if ident.is_empty()
                || !ident
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            {
                return false;
            }
        }
    }
    let (version_core, prerelease) = match version_pre.split_once('-') {
        Some((v, p)) => (v, Some(p)),
        None => (version_pre, None),
    };
    if let Some(p) = prerelease {
        if p.is_empty() {
            return false;
        }
        for ident in p.split('.') {
            if ident.is_empty()
                || !ident
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
            {
                return false;
            }
            if ident.bytes().all(|c| c.is_ascii_digit())
                && ident.len() > 1
                && ident.starts_with('0')
            {
                return false;
            }
        }
    }
    let parts: Vec<&str> = version_core.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    for part in parts {
        if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        if part.len() > 1 && part.starts_with('0') {
            return false;
        }
    }
    true
}

fn is_valid_base64(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return true;
    }
    if bytes.len() % 4 != 0 {
        return false;
    }
    let mut padding = 0;
    if bytes[bytes.len() - 1] == b'=' {
        padding += 1;
        if bytes[bytes.len() - 2] == b'=' {
            padding += 1;
        }
    }
    let data_len = bytes.len() - padding;
    for &b in &bytes[..data_len] {
        let is_b64_char = matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/');
        if !is_b64_char {
            return false;
        }
    }
    true
}

fn is_valid_e164(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes.len() > 16 {
        return false;
    }
    if bytes[0] != b'+' {
        return false;
    }
    if !matches!(bytes[1], b'1'..=b'9') {
        return false;
    }
    bytes[2..].iter().all(u8::is_ascii_digit)
}

fn is_valid_base64_url(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    let trimmed = s.trim_end_matches('=');
    trimmed
        .bytes()
        .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
}

fn is_valid_jwt(s: &str) -> bool {
    let mut parts = s.split('.');
    let p1 = match parts.next() {
        Some(part) if !part.is_empty() => part,
        _ => return false,
    };
    let p2 = match parts.next() {
        Some(part) if !part.is_empty() => part,
        _ => return false,
    };
    let p3 = match parts.next() {
        Some(part) => part,
        None => return false,
    };
    if parts.next().is_some() {
        return false;
    }
    is_valid_base64_url(p1) && is_valid_base64_url(p2) && (p3.is_empty() || is_valid_base64_url(p3))
}

fn is_valid_mac_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() == 17 {
        let sep = bytes[2];
        if (sep == b':' || sep == b'-')
            && bytes[5] == sep
            && bytes[8] == sep
            && bytes[11] == sep
            && bytes[14] == sep
        {
            return [0, 1, 3, 4, 6, 7, 9, 10, 12, 13, 15, 16]
                .iter()
                .all(|&idx| bytes[idx].is_ascii_hexdigit());
        }
    } else if bytes.len() == 14 && bytes[4] == b'.' && bytes[9] == b'.' {
        return [0, 1, 2, 3, 5, 6, 7, 8, 10, 11, 12, 13]
            .iter()
            .all(|&idx| bytes[idx].is_ascii_hexdigit());
    } else if bytes.len() == 23 && (bytes[2] == b':' || bytes[2] == b'-') {
        let sep = bytes[2];
        for i in 0..8 {
            if !bytes[i * 3].is_ascii_hexdigit() || !bytes[i * 3 + 1].is_ascii_hexdigit() {
                return false;
            }
            if i < 7 && bytes[i * 3 + 2] != sep {
                return false;
            }
        }
        return true;
    }
    false
}

fn is_valid_mime_type(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let main_part = s.split(';').next().unwrap_or(s).trim();
    let (type_part, subtype_part) = match main_part.split_once('/') {
        Some((t, st)) => (t.trim(), st.trim()),
        None => return false,
    };
    if type_part.is_empty() || subtype_part.is_empty() {
        return false;
    }
    let is_token_char = |b: u8| {
        b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
            )
    };
    type_part.bytes().all(is_token_char) && subtype_part.bytes().all(is_token_char)
}

fn is_valid_hex_digest(s: &str, expected_len: usize) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == expected_len && bytes.iter().all(u8::is_ascii_hexdigit)
}

fn has_no_control_characters(s: &str) -> bool {
    s.bytes().all(|b| !matches!(b, 0..=31 | 127))
}

fn is_valid_hostname(s: &str, rfc_1123: bool) -> bool {
    let s = s.strip_suffix('.').unwrap_or(s);
    if s.is_empty() || s.len() > 253 {
        return false;
    }
    for label in s.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        let bytes = label.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        let first_valid = if rfc_1123 {
            first.is_ascii_alphanumeric()
        } else {
            first.is_ascii_alphabetic()
        };
        if !first_valid || !last.is_ascii_alphanumeric() {
            return false;
        }
        if !bytes
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return false;
        }
    }
    true
}

fn is_valid_domain(s: &str) -> bool {
    let clean = s.strip_suffix('.').unwrap_or(s);
    if !is_valid_hostname(clean, true) {
        return false;
    }
    let labels: Vec<&str> = clean.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_valid_fqdn(s: &str) -> bool {
    if s.is_empty() || s.len() > 254 {
        return false;
    }
    let clean = s.strip_suffix('.').unwrap_or(s);
    if !is_valid_hostname(clean, true) {
        return false;
    }
    let labels: Vec<&str> = clean.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_valid_port_str(s: &str) -> bool {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    match s.parse::<u32>() {
        Ok(p) => (1..=65535).contains(&p),
        Err(_) => false,
    }
}

fn is_valid_cidr(s: &str) -> bool {
    let (ip_str, prefix_str) = match s.split_once('/') {
        Some((ip, prefix)) => (ip, prefix),
        None => return false,
    };
    if prefix_str.is_empty() || !prefix_str.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let prefix = match prefix_str.parse::<u8>() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if Ipv4Addr::from_str(ip_str).is_ok() {
        prefix <= 32
    } else if Ipv6Addr::from_str(ip_str).is_ok() {
        prefix <= 128
    } else {
        false
    }
}

fn is_valid_data_uri(s: &str) -> bool {
    let rest = match s.strip_prefix("data:") {
        Some(r) => r,
        None => return false,
    };
    let (header, data) = match rest.split_once(',') {
        Some((h, d)) => (h, d),
        None => return false,
    };
    if let Some(mime) = header.strip_suffix(";base64") {
        if !mime.is_empty() && !is_valid_mime_type(mime) {
            return false;
        }
        is_valid_base64(data)
    } else {
        if !header.is_empty() {
            let mime = header.split(';').next().unwrap_or(header);
            if !mime.is_empty() && !is_valid_mime_type(mime) {
                return false;
            }
        }
        true
    }
}

fn is_valid_latitude_str(s: &str) -> bool {
    match s.parse::<f64>() {
        Ok(lat) => lat.is_finite() && (-90.0..=90.0).contains(&lat),
        Err(_) => false,
    }
}

fn is_valid_longitude_str(s: &str) -> bool {
    match s.parse::<f64>() {
        Ok(lon) => lon.is_finite() && (-180.0..=180.0).contains(&lon),
        Err(_) => false,
    }
}

fn is_valid_coordinates_str(s: &str) -> bool {
    let (lat_str, lon_str) = match s.split_once(',') {
        Some((lat, lon)) => (lat.trim(), lon.trim()),
        None => return false,
    };
    is_valid_latitude_str(lat_str) && is_valid_longitude_str(lon_str)
}

fn is_valid_postal_code(s: &str, country: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    match country {
        "US" | "us" => {
            let bytes = trimmed.as_bytes();
            if bytes.len() == 5 {
                bytes.iter().all(u8::is_ascii_digit)
            } else if bytes.len() == 10 && bytes[5] == b'-' {
                bytes[..5].iter().all(u8::is_ascii_digit)
                    && bytes[6..].iter().all(u8::is_ascii_digit)
            } else {
                false
            }
        }
        "CA" | "ca" => {
            let cleaned: Vec<u8> = trimmed
                .bytes()
                .filter(|&b| !b.is_ascii_whitespace())
                .collect();
            if cleaned.len() != 6 {
                return false;
            }
            cleaned[0].is_ascii_alphabetic()
                && cleaned[1].is_ascii_digit()
                && cleaned[2].is_ascii_alphabetic()
                && cleaned[3].is_ascii_digit()
                && cleaned[4].is_ascii_alphabetic()
                && cleaned[5].is_ascii_digit()
        }
        "GB" | "gb" | "UK" | "uk" => {
            let cleaned: Vec<u8> = trimmed
                .bytes()
                .filter(|&b| !b.is_ascii_whitespace())
                .collect();
            if cleaned.len() < 5 || cleaned.len() > 7 {
                return false;
            }
            let len = cleaned.len();
            cleaned[len - 3].is_ascii_digit()
                && cleaned[len - 2].is_ascii_alphabetic()
                && cleaned[len - 1].is_ascii_alphabetic()
                && cleaned[..len - 3].iter().all(u8::is_ascii_alphanumeric)
        }
        "DE" | "de" | "FR" | "fr" | "IT" | "it" | "ES" | "es" => {
            trimmed.len() == 5 && trimmed.bytes().all(|b| b.is_ascii_digit())
        }
        "AU" | "au" => trimmed.len() == 4 && trimmed.bytes().all(|b| b.is_ascii_digit()),
        "NL" | "nl" => {
            let cleaned: Vec<u8> = trimmed
                .bytes()
                .filter(|&b| !b.is_ascii_whitespace())
                .collect();
            cleaned.len() == 6
                && cleaned[..4].iter().all(u8::is_ascii_digit)
                && cleaned[4..].iter().all(u8::is_ascii_alphabetic)
        }
        _ => {
            let len = trimmed.len();
            (3..=10).contains(&len)
                && trimmed
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b' ')
                && trimmed.bytes().any(|b| b.is_ascii_digit())
        }
    }
}

fn is_valid_iban(s: &str) -> bool {
    let bytes: Vec<u8> = s
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace())
        .map(|b| b.to_ascii_uppercase())
        .collect();
    if bytes.len() < 15 || bytes.len() > 34 {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic()
        || !bytes[1].is_ascii_alphabetic()
        || !bytes[2].is_ascii_digit()
        || !bytes[3].is_ascii_digit()
    {
        return false;
    }
    if !bytes.iter().all(u8::is_ascii_alphanumeric) {
        return false;
    }

    let rearranged = [&bytes[4..], &bytes[..4]].concat();
    let mut remainder = 0u32;
    for &b in &rearranged {
        if b.is_ascii_digit() {
            remainder = (remainder * 10 + (b - b'0') as u32) % 97;
        } else if b.is_ascii_alphabetic() {
            let val = (b - b'A' + 10) as u32;
            remainder = (remainder * 10 + (val / 10)) % 97;
            remainder = (remainder * 10 + (val % 10)) % 97;
        } else {
            return false;
        }
    }
    remainder == 1
}

fn is_valid_bic(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 8 && bytes.len() != 11 {
        return false;
    }
    bytes[0..4].iter().all(u8::is_ascii_alphabetic)
        && bytes[4..6].iter().all(u8::is_ascii_alphabetic)
        && bytes[6..8].iter().all(u8::is_ascii_alphanumeric)
        && (bytes.len() == 8 || bytes[8..11].iter().all(u8::is_ascii_alphanumeric))
}

fn is_valid_isbn10(s: &str) -> bool {
    let cleaned: Vec<u8> = s
        .bytes()
        .filter(|&b| b != b'-' && !b.is_ascii_whitespace())
        .collect();
    if cleaned.len() != 10 {
        return false;
    }
    let mut sum = 0u32;
    for (i, &b) in cleaned[..9].iter().enumerate() {
        if !b.is_ascii_digit() {
            return false;
        }
        sum += (10 - i as u32) * (b - b'0') as u32;
    }
    let last = cleaned[9];
    if last == b'X' || last == b'x' {
        sum += 10;
    } else if last.is_ascii_digit() {
        sum += (last - b'0') as u32;
    } else {
        return false;
    }
    sum % 11 == 0
}

fn is_valid_isbn13(s: &str) -> bool {
    let cleaned: Vec<u8> = s
        .bytes()
        .filter(|&b| b != b'-' && !b.is_ascii_whitespace())
        .collect();
    if cleaned.len() != 13 || !cleaned.iter().all(u8::is_ascii_digit) {
        return false;
    }
    if !cleaned.starts_with(b"978") && !cleaned.starts_with(b"979") {
        return false;
    }
    let mut sum = 0u32;
    for (i, item) in cleaned[..12].iter().enumerate() {
        let weight = if i % 2 == 0 { 1 } else { 3 };
        sum += weight * (item - b'0') as u32;
    }
    let check = (10 - (sum % 10)) % 10;
    check == (cleaned[12] - b'0') as u32
}

fn is_valid_isbn(s: &str) -> bool {
    is_valid_isbn10(s) || is_valid_isbn13(s)
}

fn is_valid_isin(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 12 {
        return false;
    }
    if !bytes[0].is_ascii_alphabetic()
        || !bytes[1].is_ascii_alphabetic()
        || !bytes[11].is_ascii_digit()
    {
        return false;
    }
    if !bytes[2..11].iter().all(u8::is_ascii_alphanumeric) {
        return false;
    }
    let mut digits = Vec::with_capacity(24);
    for &b in bytes {
        if b.is_ascii_digit() {
            digits.push(b - b'0');
        } else if b.is_ascii_alphabetic() {
            let val = b.to_ascii_uppercase() - b'A' + 10;
            digits.push(val / 10);
            digits.push(val % 10);
        } else {
            return false;
        }
    }
    let mut sum = 0u32;
    let mut alternate = false;
    for &d in digits.iter().rev() {
        let mut n = d as u32;
        if alternate {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alternate = !alternate;
    }
    sum % 10 == 0
}

fn is_valid_cuid(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 25
        && bytes[0] == b'c'
        && bytes[1..]
            .iter()
            .all(|&b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

fn is_valid_cuid2(s: &str) -> bool {
    let bytes = s.as_bytes();
    (24..=32).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|&b| b.is_ascii_digit() || b.is_ascii_lowercase())
}

fn is_valid_nanoid(s: &str, min_len: usize, max_len: usize) -> bool {
    let len = s.len();
    (min_len..=max_len).contains(&len)
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_valid_country_code(s: &str) -> bool {
    (s.len() == 2 || s.len() == 3) && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_valid_currency_code(s: &str) -> bool {
    s.len() == 3 && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_valid_hex(s: &str) -> bool {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn count_words(s: &str) -> usize {
    s.split_whitespace().count()
}

fn normalize_email_str(s: &str) -> String {
    let trimmed = s.trim();
    if let Some((user, domain)) = trimmed.split_once('@') {
        format!("{user}@{}", domain.to_lowercase())
    } else {
        trimmed.to_string()
    }
}

fn escape_html_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(feature = "schema-color")]
fn is_valid_rgb(s: &str, require_alpha: bool, allow_alpha: bool) -> bool {
    let trimmed = s.trim();
    let body = if require_alpha {
        match trimmed
            .strip_prefix("rgba(")
            .and_then(|t| t.strip_suffix(')'))
        {
            Some(b) => b.trim(),
            None => return false,
        }
    } else if let Some(b) = trimmed
        .strip_prefix("rgb(")
        .and_then(|t| t.strip_suffix(')'))
    {
        b.trim()
    } else if allow_alpha {
        match trimmed
            .strip_prefix("rgba(")
            .and_then(|t| t.strip_suffix(')'))
        {
            Some(b) => b.trim(),
            None => return false,
        }
    } else {
        return false;
    };

    if body.contains(',') {
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if require_alpha {
            if parts.len() != 4 {
                return false;
            }
            parts[..3].iter().all(|p| is_valid_rgb_component(p))
                && is_valid_alpha_component(parts[3])
        } else if parts.len() == 3 {
            parts.iter().all(|p| is_valid_rgb_component(p))
        } else if allow_alpha && parts.len() == 4 {
            parts[..3].iter().all(|p| is_valid_rgb_component(p))
                && is_valid_alpha_component(parts[3])
        } else {
            false
        }
    } else {
        let (rgb_part, alpha_part) = match body.split_once('/') {
            Some((rgb, alpha)) => (rgb.trim(), Some(alpha.trim())),
            None => (body, None),
        };
        let parts: Vec<&str> = rgb_part.split_whitespace().collect();
        if parts.len() != 3 || !parts.iter().all(|p| is_valid_rgb_component(p)) {
            return false;
        }
        match (alpha_part, require_alpha, allow_alpha) {
            (Some(alpha), _, true) | (Some(alpha), true, _) => is_valid_alpha_component(alpha),
            (None, false, _) => true,
            _ => false,
        }
    }
}

#[cfg(feature = "schema-color")]
fn is_valid_rgb_component(s: &str) -> bool {
    if let Some(pct) = s.strip_suffix('%') {
        match pct.parse::<f64>() {
            Ok(v) => v.is_finite() && (0.0..=100.0).contains(&v),
            Err(_) => false,
        }
    } else {
        match s.parse::<u32>() {
            Ok(v) => v <= 255,
            Err(_) => false,
        }
    }
}

#[cfg(feature = "schema-color")]
fn is_valid_alpha_component(s: &str) -> bool {
    if let Some(pct) = s.strip_suffix('%') {
        match pct.parse::<f64>() {
            Ok(v) => v.is_finite() && (0.0..=100.0).contains(&v),
            Err(_) => false,
        }
    } else {
        match s.parse::<f64>() {
            Ok(v) => v.is_finite() && (0.0..=1.0).contains(&v),
            Err(_) => false,
        }
    }
}

#[cfg(feature = "schema-color")]
fn is_valid_hsl(s: &str, require_alpha: bool, allow_alpha: bool) -> bool {
    let trimmed = s.trim();
    let body = if require_alpha {
        match trimmed
            .strip_prefix("hsla(")
            .and_then(|t| t.strip_suffix(')'))
        {
            Some(b) => b.trim(),
            None => return false,
        }
    } else if let Some(b) = trimmed
        .strip_prefix("hsl(")
        .and_then(|t| t.strip_suffix(')'))
    {
        b.trim()
    } else if allow_alpha {
        match trimmed
            .strip_prefix("hsla(")
            .and_then(|t| t.strip_suffix(')'))
        {
            Some(b) => b.trim(),
            None => return false,
        }
    } else {
        return false;
    };

    if body.contains(',') {
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if require_alpha {
            if parts.len() != 4 {
                return false;
            }
            is_valid_hue_component(parts[0])
                && is_valid_percentage_component(parts[1])
                && is_valid_percentage_component(parts[2])
                && is_valid_alpha_component(parts[3])
        } else if parts.len() == 3 {
            is_valid_hue_component(parts[0])
                && is_valid_percentage_component(parts[1])
                && is_valid_percentage_component(parts[2])
        } else if allow_alpha && parts.len() == 4 {
            is_valid_hue_component(parts[0])
                && is_valid_percentage_component(parts[1])
                && is_valid_percentage_component(parts[2])
                && is_valid_alpha_component(parts[3])
        } else {
            false
        }
    } else {
        let (hsl_part, alpha_part) = match body.split_once('/') {
            Some((hsl, alpha)) => (hsl.trim(), Some(alpha.trim())),
            None => (body, None),
        };
        let parts: Vec<&str> = hsl_part.split_whitespace().collect();
        if parts.len() != 3
            || !is_valid_hue_component(parts[0])
            || !is_valid_percentage_component(parts[1])
            || !is_valid_percentage_component(parts[2])
        {
            return false;
        }
        match (alpha_part, require_alpha, allow_alpha) {
            (Some(alpha), _, true) | (Some(alpha), true, _) => is_valid_alpha_component(alpha),
            (None, false, _) => true,
            _ => false,
        }
    }
}

#[cfg(feature = "schema-color")]
fn is_valid_hue_component(s: &str) -> bool {
    let clean = s.strip_suffix("deg").unwrap_or(s);
    match clean.parse::<f64>() {
        Ok(v) => v.is_finite() && (0.0..=360.0).contains(&v),
        Err(_) => false,
    }
}

#[cfg(feature = "schema-color")]
fn is_valid_percentage_component(s: &str) -> bool {
    match s.strip_suffix('%') {
        Some(pct) => match pct.parse::<f64>() {
            Ok(v) => v.is_finite() && (0.0..=100.0).contains(&v),
            Err(_) => false,
        },
        None => false,
    }
}

#[cfg(feature = "schema-crypto")]
fn is_valid_ethereum_address(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() != 42 {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if bytes[0] != b'0' || (bytes[1] != b'x' && bytes[1] != b'X') {
        return false;
    }
    bytes[2..].iter().all(u8::is_ascii_hexdigit)
}

#[cfg(feature = "schema-crypto")]
fn is_valid_bitcoin_address(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    let bytes = trimmed.as_bytes();
    if (trimmed.starts_with("bc1") || trimmed.starts_with("BC1"))
        && (14..=90).contains(&trimmed.len())
    {
        let data = &bytes[3..];
        return data.iter().all(|&b| {
            b.is_ascii_alphanumeric()
                && b != b'1'
                && b != b'b'
                && b != b'i'
                && b != b'o'
                && b != b'B'
                && b != b'I'
                && b != b'O'
        });
    }
    if (bytes[0] == b'1' || bytes[0] == b'3') && (26..=35).contains(&trimmed.len()) {
        return bytes.iter().all(|&b| {
            b.is_ascii_alphanumeric() && b != b'0' && b != b'O' && b != b'I' && b != b'l'
        });
    }
    false
}

#[cfg(feature = "schema-business")]
fn is_valid_eu_vat(s: &str) -> bool {
    let cleaned: Vec<u8> = s
        .bytes()
        .filter(|&b| !b.is_ascii_whitespace() && b != b'-' && b != b'.')
        .collect();
    if cleaned.len() < 8 || cleaned.len() > 14 {
        return false;
    }
    const EU_PREFIXES: [&[u8; 2]; 28] = [
        b"AT", b"BE", b"BG", b"CY", b"CZ", b"DE", b"DK", b"EE", b"EL", b"GR", b"ES", b"FI", b"FR",
        b"HR", b"HU", b"IE", b"IT", b"LT", b"LU", b"LV", b"MT", b"NL", b"PL", b"PT", b"RO", b"SE",
        b"SI", b"SK",
    ];
    let prefix = [
        cleaned[0].to_ascii_uppercase(),
        cleaned[1].to_ascii_uppercase(),
    ];
    if !EU_PREFIXES.contains(&&prefix) {
        return false;
    }
    cleaned[2..].iter().all(u8::is_ascii_alphanumeric)
}

#[cfg(feature = "schema-business")]
fn is_valid_ein(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() == 10 && bytes[2] == b'-' {
        bytes[..2].iter().all(u8::is_ascii_digit) && bytes[3..].iter().all(u8::is_ascii_digit)
    } else if bytes.len() == 9 {
        bytes.iter().all(u8::is_ascii_digit)
    } else {
        false
    }
}

#[cfg(feature = "schema-business")]
fn is_valid_ssn(s: &str) -> bool {
    let bytes = s.as_bytes();
    let (area, group, serial) = if bytes.len() == 11 && bytes[3] == b'-' && bytes[6] == b'-' {
        (&bytes[..3], &bytes[4..6], &bytes[7..11])
    } else if bytes.len() == 9 {
        (&bytes[..3], &bytes[3..5], &bytes[5..9])
    } else {
        return false;
    };
    if !area.iter().all(u8::is_ascii_digit)
        || !group.iter().all(u8::is_ascii_digit)
        || !serial.iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    if area == b"000" || area == b"666" || area[0] == b'9' {
        return false;
    }
    if group == b"00" || serial == b"0000" {
        return false;
    }
    true
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_mongodb_object_id(s: &str) -> bool {
    s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_bcp47(s: &str) -> bool {
    if s.is_empty() || s.len() > 35 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.is_empty() {
        return false;
    }
    let primary = parts[0];
    if (primary.len() != 2 && primary.len() != 3)
        || !primary.bytes().all(|b| b.is_ascii_alphabetic())
    {
        return false;
    }
    for subtag in &parts[1..] {
        if subtag.is_empty()
            || subtag.len() > 8
            || !subtag.bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return false;
        }
    }
    true
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_cron(s: &str) -> bool {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 5 || parts.len() > 6 {
        return false;
    }
    for part in parts {
        if part.is_empty() {
            return false;
        }
        let valid = part.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'*' | b'/' | b',' | b'-' | b'?' | b'L' | b'W' | b'#')
        });
        if !valid {
            return false;
        }
    }
    true
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_ksuid(s: &str) -> bool {
    s.len() == 27 && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_issn(s: &str) -> bool {
    let cleaned: Vec<u8> = s
        .bytes()
        .filter(|&b| b != b'-' && !b.is_ascii_whitespace())
        .collect();
    if cleaned.len() != 8 {
        return false;
    }
    let mut sum = 0u32;
    for (i, &b) in cleaned[..7].iter().enumerate() {
        if !b.is_ascii_digit() {
            return false;
        }
        sum += (8 - i as u32) * (b - b'0') as u32;
    }
    let last = cleaned[7];
    let check = if last == b'X' || last == b'x' {
        10
    } else if last.is_ascii_digit() {
        (last - b'0') as u32
    } else {
        return false;
    };
    let remainder = sum % 11;
    let expected = if remainder == 0 { 0 } else { 11 - remainder };
    expected == check
}

#[cfg(feature = "schema-identifiers")]
fn is_valid_imei(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.len() != 15 || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let bytes = trimmed.as_bytes();
    let mut sum = 0u32;
    for (i, &b) in bytes.iter().enumerate() {
        let mut d = (b - b'0') as u32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}
