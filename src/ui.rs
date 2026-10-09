use crate::sim::{Game, Phase};
pub fn draw(
    ctx: &egui::Context,
    game: &mut Game,
    paused: &mut bool,
    notice: &mut String,
    fps: f32,
    adapter: &str,
) {
    egui::TopBottomPanel::top("hud").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("HOT VAMPIRES");
            ui.separator();
            ui.label(format!("WAVE {} / {}", game.wave, game.config.waves));
            ui.label(format!("HP {:.0} / {:.0}", game.hp, game.stats().hp));
            ui.label(format!("{} coins", game.currency));
            ui.label(format!(
                "{:.1}s remaining",
                (game.config.wave_seconds - game.wave_time).max(0.0)
            ));
            ui.label(format!("TIME ×{:.2}", game.scale));
        });
    });
    egui::TopBottomPanel::bottom("help").show(ctx,|ui|{ui.label("WASD move · mouse motion or Q/E orbit · wheel zoom · Esc pause / free cursor · F5 load checkpoint · R restart on results");ui.small(format!("{fps:.0} FPS | enemies {} | shots {} | debris {} / {} | world {:.1}s / real {:.1}s | seed {} | {}",game.enemies.len(),game.shots.len(),game.debris.len(),game.config.debris_budget,game.world,game.real,game.seed,adapter));});
    if game.phase == Phase::Shop {
        let mut changed = false;
        egui::Window::new("Between waves — build workshop").collapsible(false).resizable(false).default_width(690.0).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui| {
            ui.heading(format!("Wave {} cleared · {} coins",game.wave,game.currency));
            ui.label("The world is paused. Buy upgrades, lock future offers, or sell your equipment.");
            ui.horizontal(|ui|{for i in 0..4 {ui.vertical(|ui|{ui.set_width(155.0);
                if let Some(c)=game.shop.offers[i].id.as_ref().and_then(|id|game.cards.iter().find(|c|&c.id==id)).cloned() {
                    ui.group(|ui|{ui.strong(&c.name);ui.small(format!("{} · common",c.category));ui.small(format!("{} coins · level {}/{}",c.price,game.level(&c.id),c.max_level));ui.label(&c.description);ui.small(c.tags.join(" / "));
                        let allowed=game.can_buy(&c);let button=ui.add_enabled(allowed.is_ok(),egui::Button::new("Buy / upgrade"));
                        let before=game.stats();let mut preview=game.clone();preview.currency=u32::MAX;let _=preview.buy(i);let after=preview.stats();
                        let button=button.on_hover_text(format!("HP {:.0} → {:.0}\nDamage ×{:.2} → ×{:.2}\nAttack speed ×{:.2} → ×{:.2}\nMove {:.2} → {:.2}\nRange {:.2} → {:.2}\n{}",before.hp,after.hp,before.damage,after.damage,before.attack_speed,after.attack_speed,before.speed,after.speed,before.range,after.range,allowed.err().unwrap_or_default()));
                        if c.id=="light_frame" {ctx.data_mut(|data|data.insert_temp(egui::Id::new("smoke-buy"),button.rect));}
                        if button.clicked(){let r=game.buy(i);*notice=r.map_or_else(|e|e,|_|format!("Bought {}",c.name));changed=true;}
                        let locked=game.shop.offers[i].locked;if ui.button(if locked{"Unlock"}else{"Lock"}).clicked(){let _=game.lock(i);changed=true;}
                    });
                } else {ui.group(|ui|{ui.label("Empty offer");ui.small("Reroll to refill");});}
            });}});
            ui.separator();ui.horizontal(|ui|{
                if ui.add_enabled(game.currency>=game.reroll_price(),egui::Button::new(format!("Reroll · {} coins",game.reroll_price()))).clicked(){*notice=game.reroll().map_or_else(|e|e,|_|"Offers refreshed".into());changed=true;}
                if ui.add_enabled(game.currency>=15 && game.hp<game.stats().hp,egui::Button::new("Heal +25 HP · 15 coins")).clicked(){*notice=game.heal().map_or_else(|e|e,|_|"Healed".into());changed=true;}
                let start=ui.button("Start next wave →");ctx.data_mut(|data|data.insert_temp(egui::Id::new("smoke-start"),start.rect));
                if start.clicked(){let _=game.start_wave();}
            });
            ui.separator();ui.label("Owned equipment — refunds are floor(40% of actual spend)");
            let owned=game.build.clone();ui.horizontal_wrapped(|ui|{for o in owned {if ui.button(format!("Sell {} L{} · +{}",o.id,o.level,o.spent*2/5)).clicked(){*notice=game.sell(&o.id).map_or_else(|e|e,|_|"Sold; stats recalculated".into());changed=true;}}});
            let s=game.stats();ui.label(format!("Move {:.1} · Damage ×{:.2} · Rate ×{:.2} · Armor {:.0} · Range {:.1} · Regen {:.1}/world s",s.speed,s.damage,s.attack_speed,s.armor,s.range,s.regen));ui.label(notice.as_str());
        });
        if changed
            && game.phase == Phase::Shop
            && let Err(e) = game.save(std::path::Path::new("saves/checkpoint.json"))
        {
            *notice = e;
        }
    }
    if *paused {
        egui::Window::new("Paused")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Standing still slows combat to 3%; pause fully stops it.");
                if ui.button("Resume").clicked() {
                    *paused = false;
                }
            });
    }
    if matches!(game.phase, Phase::Victory | Phase::Dead) {
        egui::Window::new(if game.phase == Phase::Victory {
            "RUN COMPLETE"
        } else {
            "CONSTRUCT DOWN"
        })
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading(format!("{} kills · wave {}", game.metrics.kills, game.wave));
            ui.label(format!(
                "World {:.1}s · real {:.1}s · {} purchases",
                game.world, game.real, game.metrics.purchases
            ));
            if ui.button("New run").clicked() {
                *game = Game::new(game.seed, game.config.clone());
            }
        });
    }
}
