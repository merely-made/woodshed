use redshank_model::{RepresentationIdentity, RepresentationReceipt};

const URL: &str = "https://media.example/episode.mp3";
const OTHER_URL: &str = "https://mirror.example/episode.mp3";

fn receipt(
    final_url: Option<&str>,
    etag: Option<&str>,
    digest: Option<&str>,
    byte_length: Option<u64>,
) -> RepresentationReceipt {
    RepresentationReceipt {
        final_url: final_url.map(str::to_owned),
        etag: etag.map(str::to_owned),
        complete_digest: digest.map(str::to_owned),
        byte_length,
        ..RepresentationReceipt::default()
    }
}

fn assert_identity(
    label: &str,
    left: &RepresentationReceipt,
    right: &RepresentationReceipt,
    expected: RepresentationIdentity,
) {
    assert_eq!(left.compare(right), expected, "{label}");
    assert_eq!(right.compare(left), expected, "{label} (reversed)");
}

#[test]
fn complete_digests_take_precedence_over_server_metadata() {
    use RepresentationIdentity::{Different, Same, Unproven};

    let cases = [
        (
            "equal digest survives changed URL and ETag",
            receipt(Some(URL), Some("\"old\""), Some("blake3:abc"), None),
            receipt(Some(OTHER_URL), Some("\"new\""), Some("blake3:abc"), None),
            Same,
        ),
        (
            "equal digest takes precedence over contradictory length metadata",
            receipt(None, None, Some("blake3:abc"), Some(100)),
            receipt(None, None, Some("blake3:abc"), Some(200)),
            Same,
        ),
        (
            "unequal digests override an unchanged strong validator",
            receipt(Some(URL), Some("\"fixed\""), Some("blake3:abc"), Some(100)),
            receipt(Some(URL), Some("\"fixed\""), Some("blake3:def"), Some(100)),
            Different,
        ),
        (
            "one digest alone cannot establish identity",
            receipt(None, None, Some("blake3:abc"), None),
            receipt(None, None, None, None),
            Unproven,
        ),
        (
            "two empty digests are absent evidence",
            receipt(None, None, Some(""), None),
            receipt(None, None, Some(""), None),
            Unproven,
        ),
        (
            "an empty digest does not conflict with a complete digest",
            receipt(None, None, Some(""), None),
            receipt(None, None, Some("blake3:abc"), None),
            Unproven,
        ),
        (
            "missing digest still permits proof by strong validator",
            receipt(Some(URL), Some("\"fixed\""), Some("blake3:abc"), None),
            receipt(Some(URL), Some("\"fixed\""), None, None),
            Same,
        ),
        (
            "empty digest still permits proof by strong validator",
            receipt(Some(URL), Some("\"fixed\""), Some(""), None),
            receipt(Some(URL), Some("\"fixed\""), Some("blake3:abc"), None),
            Same,
        ),
    ];

    for (label, left, right, expected) in cases {
        assert_identity(label, &left, &right, expected);
    }
}

#[test]
fn only_valid_strong_etags_prove_identity_at_the_same_final_url() {
    use RepresentationIdentity::{Same, Unproven};

    let cases = [
        ("ordinary strong tag", Some("\"abc\""), Same),
        ("empty opaque tag is valid", Some("\"\""), Same),
        ("backslash is an opaque character", Some("\"a\\b\""), Same),
        ("weak tag", Some("W/\"abc\""), Unproven),
        (
            "lowercase weak prefix is malformed",
            Some("w/\"abc\""),
            Unproven,
        ),
        ("unquoted tag", Some("abc"), Unproven),
        ("missing tag", None, Unproven),
        ("empty header", Some(""), Unproven),
        ("one quote", Some("\""), Unproven),
        ("missing closing quote", Some("\"abc"), Unproven),
        ("embedded quote", Some("\"a\"b\""), Unproven),
        ("embedded space", Some("\"a b\""), Unproven),
        ("embedded tab", Some("\"a\tb\""), Unproven),
        ("embedded newline", Some("\"a\nb\""), Unproven),
        ("DEL control character", Some("\"a\u{7f}b\""), Unproven),
        ("multiple tags", Some("\"a\", \"b\""), Unproven),
    ];

    for (label, tag, expected) in cases {
        let left = receipt(Some(URL), tag, None, None);
        let right = receipt(Some(URL), tag, None, None);
        assert_identity(label, &left, &right, expected);
    }
}

#[test]
fn incomplete_evidence_distinguishes_disproof_from_uncertainty() {
    use RepresentationIdentity::{Different, Unproven};

    let cases = [
        (
            "missing receipts",
            receipt(None, None, None, None),
            receipt(None, None, None, None),
            Unproven,
        ),
        (
            "strong tag is scoped to the final URL",
            receipt(Some(URL), Some("\"fixed\""), None, None),
            receipt(Some(OTHER_URL), Some("\"fixed\""), None, None),
            Unproven,
        ),
        (
            "absent URL cannot scope a strong tag",
            receipt(None, Some("\"fixed\""), None, None),
            receipt(None, Some("\"fixed\""), None, None),
            Unproven,
        ),
        (
            "empty URL cannot scope a strong tag",
            receipt(Some(""), Some("\"fixed\""), None, None),
            receipt(Some(""), Some("\"fixed\""), None, None),
            Unproven,
        ),
        (
            "changed strong tags do not establish different bytes",
            receipt(Some(URL), Some("\"old\""), None, None),
            receipt(Some(URL), Some("\"new\""), None, None),
            Unproven,
        ),
        (
            "matching length alone is insufficient",
            receipt(Some(URL), None, None, Some(100)),
            receipt(Some(URL), None, None, Some(100)),
            Unproven,
        ),
        (
            "unequal known lengths prove different bytes",
            receipt(None, None, None, Some(100)),
            receipt(None, None, None, Some(200)),
            Different,
        ),
        (
            "unequal lengths override an unchanged strong tag",
            receipt(Some(URL), Some("\"fixed\""), None, Some(100)),
            receipt(Some(URL), Some("\"fixed\""), None, Some(200)),
            Different,
        ),
        (
            "zero is a known length",
            receipt(None, None, None, Some(0)),
            receipt(None, None, None, Some(1)),
            Different,
        ),
        (
            "one unknown length does not prove a difference",
            receipt(None, None, None, Some(100)),
            receipt(None, None, None, None),
            Unproven,
        ),
    ];

    for (label, left, right, expected) in cases {
        assert_identity(label, &left, &right, expected);
    }

    let mut left = receipt(Some(URL), None, None, Some(100));
    left.last_modified = Some("Wed, 23 Sep 2026 12:00:00 GMT".into());
    left.retrieved_at_ms = Some(1000);
    let mut right = left.clone();
    right.retrieved_at_ms = Some(2000);
    assert_identity(
        "same date and length remain unproven",
        &left,
        &right,
        Unproven,
    );

    right.last_modified = Some("Thu, 24 Sep 2026 12:00:00 GMT".into());
    assert_identity(
        "changed date alone remains unproven",
        &left,
        &right,
        Unproven,
    );
}
