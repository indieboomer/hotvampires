use crate::{content::Card, sim::Game};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Owned {
    pub id: String,
    pub level: u32,
    pub spent: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Offer {
    pub id: Option<String>,
    pub locked: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Shop {
    pub offers: Vec<Offer>,
    pub rerolls: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
    pub armor: f32,
    pub pickup: f32,
    pub crit: f32,
    pub crit_multiplier: f32,
    pub area: f32,
    pub regen: f32,
}
impl Game {
    pub fn level(&self, id: &str) -> u32 {
        self.build
            .iter()
            .find(|o| o.id == id)
            .map_or(0, |o| o.level)
    }
    pub fn stats(&self) -> Stats {
        let n = |id| self.level(id) as f32;
        Stats {
            hp: (65.0 + 15.0 * n("core") - 5.0 * n("lens")).max(10.0),
            speed: 4.8 * (1.0 + 0.12 * n("light_frame") - 0.05 * n("armor_plate")).max(0.25),
            damage: (1.0 + 0.25 * n("capacitor")).max(0.2),
            attack_speed: (1.0 + 0.15 * n("stabilizer") - 0.1 * n("capacitor")
                + self.momentum * 0.15 * n("momentum"))
            .clamp(0.2, 4.0),
            range: 8.0 * (1.0 + 0.15 * n("lens")).max(0.25),
            armor: 2.0 * n("armor_plate") - n("light_frame"),
            pickup: 1.6 + 0.7 * n("magnet"),
            crit: (0.05 + 0.08 * n("shard")).min(0.75),
            crit_multiplier: 1.5 + 0.3 * n("amplifier"),
            area: 1.0 + 0.15 * n("ring"),
            regen: 0.3 * n("regenerator"),
        }
    }
    pub fn can_buy(&self, c: &Card) -> Result<(), String> {
        if self.currency < c.price {
            return Err("Not enough coins".into());
        }
        if self.level(&c.id) >= c.max_level {
            return Err("Maximum level".into());
        }
        let slots = if c.category == "weapon" {
            3
        } else if c.category == "skill" {
            4
        } else {
            usize::MAX
        };
        if self.level(&c.id) == 0
            && self
                .build
                .iter()
                .filter(|o| {
                    self.cards
                        .iter()
                        .any(|x| x.id == o.id && x.category == c.category)
                })
                .count()
                >= slots
        {
            return Err(format!("All {} slots occupied", c.category));
        }
        Ok(())
    }
    pub fn fill_shop(&mut self) {
        while self.shop.offers.len() < 4 {
            self.shop.offers.push(Offer {
                id: None,
                locked: false,
            });
        }
        for i in 0..4 {
            if self.shop.offers[i].locked && self.shop.offers[i].id.is_some() {
                continue;
            }
            let pool: Vec<_> = self
                .cards
                .iter()
                .filter(|c| {
                    self.level(&c.id) < c.max_level
                        && !self
                            .shop
                            .offers
                            .iter()
                            .enumerate()
                            .any(|(j, o)| j != i && o.id.as_ref() == Some(&c.id))
                })
                .map(|c| c.id.clone())
                .collect();
            self.shop.offers[i] = Offer {
                id: if pool.is_empty() {
                    None
                } else {
                    Some(pool[self.shop_rng.index(pool.len())].clone())
                },
                locked: false,
            };
        }
        if self.wave == 1
            && self.shop.rerolls == 0
            && !self.shop.offers.iter().any(|o| {
                o.id.as_ref().is_some_and(|id| {
                    self.cards
                        .iter()
                        .any(|c| &c.id == id && self.can_buy(c).is_ok())
                })
            })
            && let Some(i) = self.shop.offers.iter().position(|o| !o.locked)
        {
            self.shop.offers[i].id = Some("light_frame".into());
        }
    }
    pub fn buy(&mut self, index: usize) -> Result<(), String> {
        self.require_shop()?;
        let id = self
            .shop
            .offers
            .get(index)
            .and_then(|o| o.id.clone())
            .ok_or("Empty offer")?;
        let c = self
            .cards
            .iter()
            .find(|c| c.id == id)
            .ok_or("Unknown offer")?
            .clone();
        self.can_buy(&c)?;
        self.currency -= c.price;
        if let Some(o) = self.build.iter_mut().find(|o| o.id == id) {
            o.level += 1;
            o.spent += c.price;
        } else {
            self.build.push(Owned {
                id: id.clone(),
                level: 1,
                spent: c.price,
            });
        }
        self.hp = self.hp.min(self.stats().hp);
        self.shop.offers[index] = Offer {
            id: None,
            locked: false,
        };
        self.metrics.purchases += 1;
        self.metrics.spent += c.price;
        self.event("purchase", serde_json::json!({"id":id,"price":c.price}));
        Ok(())
    }
    pub fn sell(&mut self, id: &str) -> Result<(), String> {
        self.require_shop()?;
        let i = self
            .build
            .iter()
            .position(|o| o.id == id)
            .ok_or("Not owned")?;
        if id == "needle"
            && self
                .build
                .iter()
                .filter(|o| {
                    self.cards
                        .iter()
                        .any(|c| c.id == o.id && c.category == "weapon")
                })
                .count()
                == 1
        {
            return Err("Keep at least one weapon".into());
        }
        if self
            .cards
            .iter()
            .any(|c| c.id == id && c.category == "weapon")
            && self
                .build
                .iter()
                .filter(|o| {
                    self.cards
                        .iter()
                        .any(|c| c.id == o.id && c.category == "weapon")
                })
                .count()
                == 1
        {
            return Err("Keep at least one weapon".into());
        }
        let refund = self.build.remove(i).spent * 2 / 5;
        self.currency += refund;
        self.hp = self.hp.min(self.stats().hp);
        self.event("sale", serde_json::json!({"id":id,"refund":refund}));
        Ok(())
    }
    pub fn reroll_price(&self) -> u32 {
        (5 + 5 * self.shop.rerolls)
            .saturating_sub(2 * self.level("coupon"))
            .max(1)
    }
    pub fn reroll(&mut self) -> Result<(), String> {
        self.require_shop()?;
        let cost = self.reroll_price();
        if self.currency < cost {
            return Err("Not enough coins".into());
        }
        self.currency -= cost;
        self.metrics.spent += cost;
        self.metrics.rerolls += 1;
        self.shop.rerolls += 1;
        self.fill_shop();
        self.event("reroll", serde_json::json!({"price":cost}));
        Ok(())
    }
    pub fn heal(&mut self) -> Result<(), String> {
        self.require_shop()?;
        if self.currency < 15 || self.hp >= self.stats().hp {
            return Err("Healing unavailable (15 coins)".into());
        }
        self.currency -= 15;
        self.hp = (self.hp + 25.0).min(self.stats().hp);
        self.metrics.spent += 15;
        self.metrics.heals += 1;
        self.event("heal", serde_json::json!({"price":15}));
        Ok(())
    }
    pub fn lock(&mut self, index: usize) -> Result<(), String> {
        self.require_shop()?;
        let o = self.shop.offers.get_mut(index).ok_or("Unknown offer")?;
        if o.id.is_none() {
            return Err("Empty offer".into());
        }
        o.locked = !o.locked;
        Ok(())
    }
    fn require_shop(&self) -> Result<(), String> {
        if self.phase == crate::sim::Phase::Shop {
            Ok(())
        } else {
            Err("Shop is closed".into())
        }
    }
}
