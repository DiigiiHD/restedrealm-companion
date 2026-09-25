//! Plain words for the addon's record kinds. Players never see the raw names.

/// A readable name for one record kind.
pub fn kind_label(kind: &str) -> String {
    let label = match kind {
        "entity_sighting" => "Creature or character seen",
        "gossip" => "Conversation opened",
        "gossip_confirmation" => "Conversation confirmation",
        "gossip_poi" => "Map point from a conversation",
        "gossip_selection" => "Conversation choice",
        "item_count_increase" => "Item received",
        "loot_slot_cleared" => "Item looted",
        "loot_window" => "Loot window",
        "merchant" => "Merchant stock",
        "profession_catalog" => "Profession recipes",
        "profession_opened" => "Profession window",
        "quest" => "Quest details",
        "quest_area_edge_sample" => "Quest area sample",
        "quest_greeting" => "Quest giver greeting",
        "quest_log_text" => "Quest log",
        "quest_objectives" => "Quest objectives and rewards",
        "quest_reputation" => "Quest reputation",
        "quest_state" => "Quest progress",
        "recipe" => "Recipe",
        "taxi_map" => "Flight map",
        "trainer" => "Trainer skills",
        "trainer_window" => "Trainer window",
        other => {
            let words = other.replace('_', " ");
            let mut chars = words.chars();
            return match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => "Other".into(),
            };
        }
    };
    label.into()
}

/// The group a kind counts towards in "Recorded this week".
pub fn kind_group(kind: &str) -> &'static str {
    match kind {
        k if k.starts_with("quest") => "Quests",
        k if k.starts_with("gossip") || k == "entity_sighting" => "Creatures and NPCs",
        "merchant" | "trainer" | "trainer_window" => "Merchants and trainers",
        "loot_window" | "loot_slot_cleared" | "item_count_increase" => "Items and loot",
        "profession_catalog" | "profession_opened" | "recipe" => "Professions",
        "taxi_map" => "Travel",
        _ => "Other",
    }
}

pub const GROUP_ORDER: &[&str] =
    &["Quests", "Creatures and NPCs", "Merchants and trainers", "Items and loot", "Professions", "Travel", "Other"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reads_as_words() {
        assert_eq!(kind_label("quest_objectives"), "Quest objectives and rewards");
        assert_eq!(kind_label("new_thing"), "New thing");
        assert_eq!(kind_group("quest_state"), "Quests");
        assert_eq!(kind_group("gossip_poi"), "Creatures and NPCs");
        assert!(GROUP_ORDER.contains(&kind_group("anything")));
    }
}
