//! A basic example of how a clicker game can be started

use std::time::Duration;

use approximint::Approximint;
use cushy::value::{Dynamic, IntoReader, MapEach, Source};
use cushy::widget::{MakeWidget, WidgetInstance};
use cushy::Run;
use cushy_clicker::{ResourcePool, Upgrade};

fn main() -> cushy::Result {
    let game = Game::default();

    // Our game will generate resources automatically every 100ms.
    game.resource.every(Duration::from_millis(100), {
        let game = game.clone();
        move |mut resource| {
            game.tick(&mut resource);
        }
    });

    // A boolean controlling whether we format as scientific notation or words.
    let scientific = Dynamic::new(false);
    // Create the label that displays our current resource total.
    let resource_label = (&*game.resource, &scientific)
        .map_each(|(value, scientific)| {
            if *scientific {
                value.as_scientific().to_string()
            } else {
                value.as_english().to_string()
            }
        })
        .to_label()
        .h1()
        .centered();

    resource_label
        // Show a checkbox controlling displaying using scientific notation.
        .and("Scientific".into_checkbox(scientific).centered())
        // A button that adds 1 to the resource every time it is clicked.
        .and(
            "Manual"
                .into_button()
                .on_click(game.resource.on_click(|mut resource| {
                    *resource += 1;
                })),
        )
        // Buttons to purchase upgrades that also have generated amounts.
        .and(upgrade_button("T1", &game.upgrades[0], &game.totals[0]))
        .and(upgrade_button("T2", &game.upgrades[1], &game.totals[1]))
        .and(upgrade_button("T3", &game.upgrades[2], &game.totals[2]))
        // The final upgrade doesn't have any generated amounts since it's the
        // last in the line.
        .and(game.upgrades[3].purchase_button({
            move |level, cost| {
                let cost = cost.expect("always upgrades");
                format!("Buy T4 for {cost} ({level})")
            }
        }))
        .into_rows()
        .centered()
        .run()
}

#[derive(Debug, Clone)]
struct Game {
    resource: ResourcePool,
    upgrades: [Upgrade; 4],
    totals: [ResourcePool; 3],
}

impl Default for Game {
    fn default() -> Self {
        let resource = ResourcePool::default();
        Self {
            // Create 4 upgrade tiers with progressively more expensive base costs. Each
            // upgrade gets 25% more expensive each tier.
            upgrades: [
                Upgrade::new(10, &resource)
                    .with_cost_fn(|_level, current_cost| Some(current_cost + current_cost * 0.25)),
                Upgrade::new(100, &resource)
                    .with_cost_fn(|_level, current_cost| Some(current_cost + current_cost * 0.25)),
                Upgrade::new(1_000, &resource)
                    .with_cost_fn(|_level, current_cost| Some(current_cost + current_cost * 0.25)),
                Upgrade::new(10_000, &resource)
                    .with_cost_fn(|_level, current_cost| Some(current_cost + current_cost * 0.25)),
            ],
            resource,
            // This example shows a clicker game where every upgrade generates the
            // previous level's resource, which in turn generates the previous level's
            // resource. We need three storage buckets for the total quantity of the
            // first three upgrade tiers.
            totals: [
                ResourcePool::default(),
                ResourcePool::default(),
                ResourcePool::default(),
            ],
        }
    }
}

impl Game {
    fn tick(&self, resource: &mut Approximint) {
        // Calculate each tier's total values while also generating new
        // resources.
        let t4 = self.upgrades[3].level().get();
        let t3 = self.upgrades[2].level().get() + self.totals[2].fetch_add(t4);
        let t2 = self.upgrades[1].level().get() + self.totals[1].fetch_add(t3);
        let t1 = self.upgrades[0].level().get() + self.totals[0].fetch_add(t2);
        *resource += t1;
    }
}

fn upgrade_button(
    label: impl Into<String>,
    upgrade: &Upgrade,
    total_owned: &ResourcePool,
) -> WidgetInstance {
    let label = label.into();
    upgrade.purchase_button_with_quantity(total_owned, move |level, total, cost| {
        let cost = cost.expect("always upgrades");
        if level == total {
            format!("Buy {label} for {cost} ({level})")
        } else {
            format!("Buy {label} for {cost} ({level} : {total})")
        }
    })
}
