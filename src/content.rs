use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub category: String,
    pub price: u32,
    pub max_level: u32,
    pub tags: Vec<String>,
    pub description: String,
}
pub fn load() -> Vec<Card> {
    let cards: Vec<Card> =
        serde_json::from_str(include_str!("../data/content.json")).expect("invalid content");
    let mut ids = std::collections::HashSet::new();
    for c in &cards {
        assert!(ids.insert(&c.id) && c.price > 0 && c.max_level > 0);
        assert!(["weapon", "skill", "item"].contains(&c.category.as_str()));
    }
    cards
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub min_scale: f32,
    pub transition_seconds: f32,
    pub wave_seconds: f32,
    pub waves: u32,
    pub spawn_seconds: f32,
    pub enemy_hp: f32,
    pub contact_damage: f32,
    pub debris_budget: usize,
}
impl Default for Config {
    fn default() -> Self {
        serde_json::from_str(include_str!("../data/config.json")).unwrap()
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(0.001..=1.0).contains(&self.min_scale)
            || !self.transition_seconds.is_finite()
            || self.transition_seconds <= 0.0
            || !self.wave_seconds.is_finite()
            || self.wave_seconds <= 0.0
            || self.waves == 0
            || self.waves > 100
            || !self.spawn_seconds.is_finite()
            || self.spawn_seconds < 0.05
            || !self.enemy_hp.is_finite()
            || self.enemy_hp <= 0.0
            || !self.contact_damage.is_finite()
            || self.contact_damage < 0.0
            || self.debris_budget > 10000
        {
            return Err("invalid configuration ranges".into());
        }
        Ok(())
    }
}
