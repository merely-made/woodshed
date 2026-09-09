#![forbid(unsafe_code)]

//! Host-neutral projection from Errand's RSS/Atom facts into Redshank records.

use errand::parse::feed;
use redshank_model::{
    FeedEpisodeFacts, FeedResource, FeedSubscription, FeedTranscript, ItemId, LibraryItem,
    MediaSource,
};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedImport {
    pub subscription: FeedSubscription,
    pub episodes: Vec<LibraryItem>,
}

pub fn import(
    body: &str,
    final_feed_url: &str,
    refreshed_at_ms: u64,
) -> Result<FeedImport, String> {
    let base = Url::parse(final_feed_url).map_err(|error| format!("invalid feed URL: {error}"))?;
    let parsed = feed::parse(body).map_err(|error| error.to_string())?;
    let title = parsed.title.clone().unwrap_or_else(|| base.to_string());
    let mut diagnostics = parsed.diagnostics.clone();
    if parsed.html_stripped > 0 {
        diagnostics.push(format!(
            "HTML was stripped from {} episode summaries",
            parsed.html_stripped
        ));
    }
    let subscription = FeedSubscription {
        feed_url: base.to_string(),
        title,
        subtitle: parsed.subtitle,
        link: resolve_optional(&base, parsed.link),
        language: parsed.lang,
        artwork: resolve_optional(&base, parsed.artwork),
        last_refreshed_ms: Some(refreshed_at_ms),
        diagnostics,
    };
    let episodes = parsed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let enclosure = entry.enclosures.into_iter().find(|candidate| {
                candidate
                    .media_type
                    .as_deref()
                    .is_none_or(|kind| kind.starts_with("audio/"))
            })?;
            let enclosure_url = resolve(&base, &enclosure.url)?;
            let guid = entry.guid.unwrap_or_else(|| enclosure_url.clone());
            let id = ItemId(format!(
                "feed:{}",
                blake3::hash(format!("{}\0{guid}", subscription.feed_url).as_bytes()).to_hex()
            ));
            Some(LibraryItem::FeedEpisode {
                id,
                feed_url: subscription.feed_url.clone(),
                guid,
                title: entry.title.unwrap_or_else(|| "Untitled episode".into()),
                source: MediaSource::Enclosure { url: enclosure_url },
                facts: Box::new(FeedEpisodeFacts {
                    published: entry.date,
                    summary: entry.summary,
                    duration: entry.duration,
                    artwork: resolve_optional(&base, entry.artwork),
                    enclosure_media_type: enclosure.media_type,
                    enclosure_byte_length: enclosure.byte_length,
                    chapters: entry
                        .chapters
                        .into_iter()
                        .filter_map(|resource| {
                            Some(FeedResource {
                                url: resolve(&base, &resource.url)?,
                                media_type: resource.media_type,
                            })
                        })
                        .collect(),
                    transcripts: entry
                        .transcripts
                        .into_iter()
                        .filter_map(|transcript| {
                            Some(FeedTranscript {
                                url: resolve(&base, &transcript.url)?,
                                media_type: transcript.media_type,
                                language: transcript.language,
                                relation: transcript.rel,
                            })
                        })
                        .collect(),
                }),
            })
        })
        .collect();
    Ok(FeedImport {
        subscription,
        episodes,
    })
}

fn resolve(base: &Url, value: &str) -> Option<String> {
    base.join(value.trim()).ok().map(Into::into)
}

fn resolve_optional(base: &Url, value: Option<String>) -> Option<String> {
    value.and_then(|value| resolve(base, &value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_retains_identity_and_podcast_resources() {
        let body = r#"<rss xmlns:podcast="https://podcastindex.org/namespace/1.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" version="2.0"><channel><title>Marsh Notes</title><item><guid>episode-7</guid><title>Bittern</title><description><![CDATA[<b>Field</b> note]]></description><enclosure url="audio/7.mp3" type="audio/mpeg" length="42"/><itunes:duration>01:02</itunes:duration><podcast:transcript url="notes/7.vtt" type="text/vtt"/><podcast:chapters url="notes/7.json" type="application/json"/></item></channel></rss>"#;
        let imported = import(body, "https://example.test/feed.xml", 77).unwrap();
        assert_eq!(imported.subscription.title, "Marsh Notes");
        assert_eq!(imported.subscription.last_refreshed_ms, Some(77));
        let LibraryItem::FeedEpisode {
            guid,
            source,
            facts,
            ..
        } = &imported.episodes[0]
        else {
            unreachable!()
        };
        assert_eq!(guid, "episode-7");
        assert_eq!(
            source.enclosure_url(),
            Some("https://example.test/audio/7.mp3")
        );
        assert_eq!(facts.enclosure_byte_length, Some(42));
        assert_eq!(facts.transcripts[0].url, "https://example.test/notes/7.vtt");
        assert_eq!(facts.chapters[0].url, "https://example.test/notes/7.json");
    }

    #[test]
    fn enclosure_url_is_a_stable_fallback_identity() {
        let body = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Atom</title><entry><title>One</title><link rel="enclosure" href="one.mp3" type="audio/mpeg"/></entry></feed>"#;
        let first = import(body, "https://example.test/podcast/atom", 1).unwrap();
        let second = import(body, "https://example.test/podcast/atom", 2).unwrap();
        assert_eq!(first.episodes[0].id(), second.episodes[0].id());
    }
}
