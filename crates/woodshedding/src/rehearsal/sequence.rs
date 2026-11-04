use super::*;

impl Set {
    /// Build a Set from ordered cards, minting an occurrence id for each.
    pub fn from_cards(cards: impl IntoIterator<Item = Card>, loop_mode: LoopMode) -> Self {
        let mut set = Self {
            loop_mode,
            ..Self::default()
        };
        for card in cards {
            set.push(card);
        }
        set
    }

    /// Hand out the next occurrence id. Reserve MAX so every published Set
    /// remains admissible on reopen; exhaustion never wraps or reuses IDs.
    pub(super) fn mint_card_id(&mut self) -> Option<CardId> {
        let next = self
            .last_card_id
            .checked_add(1)
            .filter(|id| *id < u64::MAX)?;
        self.last_card_id = next;
        Some(CardId(next))
    }

    /// Give every card an occurrence id, for a Set loaded from a session
    /// written before ids existed. Cards that already have one keep it, and
    /// the mint continues past the highest id in the file, so a partially
    /// migrated Set cannot mint a collision. Idempotent. Invalid or exhausted
    /// input stays unchanged; durable callers must use [`Self::admit_card_ids`]
    /// to report refusal.
    pub fn ensure_card_ids(&mut self) {
        let _ = self.admit_card_ids();
    }

    /// Admit persisted occurrence identities. Only legacy unassigned Cards may
    /// gain IDs: a duplicate assigned ID is ambiguous to arrangements, actions
    /// and graph references and must never be silently renumbered. Failure
    /// leaves the Set unchanged, including its mint state. The admitted mint
    /// must be below MAX even when no legacy Cards require migration.
    pub fn admit_card_ids(&mut self) -> Result<(), String> {
        let mut seen = std::collections::HashSet::new();
        let mut highest = self.last_card_id;
        let mut missing = 0u64;
        for card in &self.cards {
            if card.id.is_assigned() {
                if !seen.insert(card.id) {
                    return Err(format!("Duplicate Card identity {}", card.id.0));
                }
                highest = highest.max(card.id.0);
            } else {
                missing += 1;
            }
        }
        let migrated_highest = highest
            .checked_add(missing)
            .ok_or("Card identity migration exhausted")?;
        if migrated_highest == u64::MAX {
            return Err("Card identity mint exhausted".into());
        }
        // Preflight above proves all increments fit and stay below MAX. Publish
        // only after the whole migration has been admitted.
        self.last_card_id = highest;
        for card in &mut self.cards {
            if !card.id.is_assigned() {
                self.last_card_id += 1;
                card.id = CardId(self.last_card_id);
            }
        }
        Ok(())
    }

    /// Copy instructions under fresh occurrence identities, remapping musical
    /// alignment too. Missing references receive reserved identities, so a
    /// later insertion cannot accidentally resolve them to unrelated material.
    pub fn copy_instructions_from(&mut self, source: &Set) -> Result<(), String> {
        let mut source = source.clone();
        source.admit_card_ids()?;
        let mut copy = self.clone();
        copy.admit_card_ids()?;
        let mut mapping = std::collections::HashMap::new();
        copy.cards.clear();
        for mut card in source.cards {
            let next = copy.mint_card_id().ok_or("Card identity mint exhausted")?;
            mapping.insert(card.id, next);
            card.id = next;
            copy.cards.push(card);
        }
        let mut arrangement = source.progression_arrangement;
        if let Some(saved) = &mut arrangement {
            for binding in &mut saved.alignment.cards {
                if !binding.card.is_assigned() {
                    return Err("Alignment has an unassigned Card identity".into());
                }
                let next = match mapping.get(&binding.card) {
                    Some(next) => *next,
                    None => {
                        let next = copy.mint_card_id().ok_or("Card identity mint exhausted")?;
                        mapping.insert(binding.card, next);
                        next
                    },
                };
                binding.card = next;
            }
        }
        copy.progression_arrangement = arrangement;
        copy.music = source.music;
        copy.cursor = source.cursor.min(copy.cards.len().saturating_sub(1));
        copy.loop_mode = source.loop_mode;
        *self = copy;
        Ok(())
    }

    /// Position of the occurrence `id` in current Set order.
    pub fn index_of(&self, id: CardId) -> Option<usize> {
        if !id.is_assigned() {
            return None;
        }
        self.cards.iter().position(|card| card.id == id)
    }

    pub fn id_at(&self, index: usize) -> Option<CardId> {
        self.cards.get(index).map(|card| card.id)
    }

    /// The occurrence on the stage, by identity.
    pub fn cursor_id(&self) -> Option<CardId> {
        self.id_at(self.cursor.min(self.cards.len().saturating_sub(1)))
    }

    /// Put the cursor on `id`. Returns false if it is no longer in the Set,
    /// leaving the cursor where it was.
    pub fn select_id(&mut self, id: CardId) -> bool {
        match self.index_of(id) {
            Some(index) => {
                self.cursor = index;
                true
            },
            None => false,
        }
    }

    pub fn card(&self, id: CardId) -> Option<&Card> {
        self.index_of(id).map(|index| &self.cards[index])
    }

    pub fn card_mut(&mut self, id: CardId) -> Option<&mut Card> {
        self.index_of(id).map(|index| &mut self.cards[index])
    }

    /// Project the ordered Set as numbered Card nodes joined by typed `Next`
    /// edges. Nodes are addressed by [`CardId`]; the visible number is derived
    /// from current order. Consumers may filter edge kinds
    /// ([`SetGraph::with_relations`]) without changing Set truth.
    pub fn graph(&self) -> SetGraph {
        let nodes: Vec<SetGraphNode> = self
            .cards
            .iter()
            .enumerate()
            .map(|(index, card)| SetGraphNode {
                id: card.id,
                index,
                number: index + 1,
                label: card.label.clone(),
                kind: card.material.tag(),
            })
            .collect();
        let edges = nodes
            .windows(2)
            .map(|pair| SetGraphEdge {
                from: pair[0].id,
                to: pair[1].id,
                kind: SetGraphEdgeKind::Next,
            })
            .collect();
        SetGraph { nodes, edges }
    }

    /// Stage a card as a new occurrence. The id is minted here, so staging the
    /// same material twice yields two distinct occurrences. An exhausted mint
    /// leaves the Set unchanged.
    pub fn push(&mut self, card: Card) {
        let Some(id) = self.mint_card_id() else {
            return;
        };
        self.cards.push(Card { id, ..card });
    }

    /// Insert a copy of the card at `idx` right after it. The copy is a new
    /// occurrence with its own id; the original keeps its own. An exhausted
    /// mint leaves the Set unchanged.
    pub fn duplicate(&mut self, idx: usize) {
        if let Some(c) = self.cards.get(idx).cloned() {
            let Some(id) = self.mint_card_id() else {
                return;
            };
            self.cards.insert(idx + 1, Card { id, ..c });
        }
    }

    /// Remove the card at `idx`, keeping the cursor on a valid slot.
    pub fn remove(&mut self, idx: usize) {
        if idx >= self.cards.len() {
            return;
        }
        self.cards.remove(idx);
        if self.cursor >= self.cards.len() {
            self.cursor = self.cards.len().saturating_sub(1);
        }
    }

    /// Swap the card at `idx` with its neighbor in `dir` (-1 up, +1 down),
    /// keeping the cursor following the moved card if it was the cursor.
    pub fn move_card(&mut self, idx: usize, dir: i32) {
        let target = idx as i32 + dir;
        if idx >= self.cards.len() || target < 0 || target as usize >= self.cards.len() {
            return;
        }
        let target = target as usize;
        self.cards.swap(idx, target);
        if self.cursor == idx {
            self.cursor = target;
        } else if self.cursor == target {
            self.cursor = idx;
        }
    }
}
