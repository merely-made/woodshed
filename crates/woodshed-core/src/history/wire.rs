use super::*;

/// The flat log woodshed persisted before adopting the lineage. Read-only: it
/// exists so a saved session migrates, and it is never written again.
#[derive(Deserialize)]
struct FlatLog {
    events: Vec<PracticeEvent>,
}

impl Serialize for PracticeHistory {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.lineage.to_snapshot().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PracticeHistory {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let value = serde_json::Value::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| D::Error::custom("Expected practice history object"))?;
        // A recognized current key commits to current decoding, even if an
        // `events` field is also present. Corruption must reach host quarantine.
        if ["entries", "visits", "owners"]
            .iter()
            .any(|key| object.contains_key(*key))
        {
            let snapshot: HistorySnapshot =
                serde_json::from_value(value).map_err(D::Error::custom)?;
            validate_history_snapshot(&snapshot).map_err(D::Error::custom)?;
            Ok(Self {
                lineage: Lineage::from_snapshot(snapshot),
            })
        } else {
            let flat: FlatLog = serde_json::from_value(value).map_err(D::Error::custom)?;
            // Replay in recorded order. Each engagement keeps its kind,
            // time, and measured span exactly; the edges are rebuilt from
            // the order actually walked, which is what the flat log's
            // `from_id` recorded in every flow that produced one.
            let mut history = Self::default();
            for event in flat.events {
                history.record_observation(
                    event.at_ms,
                    event.subject_id,
                    event.kind,
                    event.from_id,
                    event.practiced_ms,
                    event.provenance,
                );
            }
            Ok(history)
        }
    }
}
