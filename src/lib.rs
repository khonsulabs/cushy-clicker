//! Utilities for creating incremental/clicker games in Rust using Cushy.
use std::ops::Deref;
use std::time::Duration;

use approximint::Approximint;
use cushy::animation::{IntoAnimate, Spawn};
use cushy::value::{
    Destination, Dynamic, DynamicGuard, IntoReader, IntoValue, MapEach, MapEachCloned, Source,
};
use cushy::widget::{MakeWidget, SharedCallback, WidgetInstance};

/// A dynamic [`Approximint`].
#[derive(Default, Debug, Clone)]
pub struct ResourcePool(Dynamic<Approximint>);

impl IntoReader<Approximint> for ResourcePool {
    fn into_reader(self) -> cushy::value::DynamicReader<Approximint> {
        self.0.into_reader()
    }
}

impl ResourcePool {
    /// Returns a new pool with the given initial value.
    pub fn new(initial: impl Into<Approximint>) -> Self {
        Self(Dynamic::new(initial.into()))
    }

    /// Fetches the currently stored value and then adds `value` to it. Returns
    /// the originally stored value.
    pub fn fetch_add(&self, value: impl Into<Approximint>) -> Approximint {
        self.0.map_mut(|mut resource| {
            let current = *resource;
            *resource += value.into();
            current
        })
    }

    /// Returns a closure that invokes `on_click` with access to the pool.
    ///
    /// The returned closure is designed to be used with
    /// [`Button::on_click`](cushy::widgets::Button::on_click).
    pub fn on_click<T>(
        &self,
        mut on_click: impl FnMut(DynamicGuard<'_, Approximint, false>) + Send + 'static,
    ) -> impl FnMut(T) + Send + 'static {
        let pool = self.0.clone();
        move |_| on_click(pool.lock())
    }

    /// Invokes `every` each time `duration` elapses.
    pub fn every(
        &self,
        duration: Duration,
        mut every: impl FnMut(DynamicGuard<'_, Approximint, false>) + Send + Sync + 'static,
    ) {
        let pool = self.clone();
        duration
            .and_then(SharedCallback::new(move |()| {
                every(pool.lock());
            }))
            .cycle()
            .spawn()
            .detach();
        // self.every_inner(duration, Arc::new(Mutex::new(every)));
    }

    /// Returns a dynamic boolean that is true when this pool's value is greater
    /// than or equal to `above`.
    pub fn when_above(&self, above: impl IntoValue<Approximint>) -> Dynamic<bool> {
        let above = above.into_value();
        self.0
            .map_each(move |value| above.map(|above| *value >= *above))
    }

    /// Returns a button that increments `self` by 1 when clicked, but only if
    /// `cost` can be deducted from `purchase_from`.
    pub fn purchase_button(
        &self,
        caption: impl Fn(Approximint) -> String + Send + 'static,
        cost: impl IntoValue<Approximint>,
        purchase_from: &ResourcePool,
    ) -> WidgetInstance {
        let purchase_from = purchase_from.clone();
        let cost = cost.into_value();
        self.0
            .map_each_cloned(caption)
            .into_button()
            .on_click(self.on_click({
                let purchase_from = purchase_from.clone();
                let cost = cost.clone();
                move |mut t4| {
                    *purchase_from.lock() -= cost.get();
                    *t4 += Approximint::ONE;
                }
            }))
            .with_enabled(purchase_from.when_above(cost))
    }
}

impl Deref for ResourcePool {
    type Target = Dynamic<Approximint>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A purchasable resource.
#[derive(Clone, Debug)]
pub struct Upgrade {
    level: ResourcePool,
    cost: Dynamic<Option<Approximint>>,
    source_pool: ResourcePool,
    cost_function: Option<SharedCallback<(Approximint, Approximint), Option<Approximint>>>,
}

impl Upgrade {
    /// Returns a new upgrade with a cost of `base_cost` deducated from
    /// `source_pool`.
    pub fn new(base_cost: impl Into<Approximint>, source_pool: &ResourcePool) -> Self {
        Self {
            level: ResourcePool::default(),
            cost: Dynamic::new(Some(base_cost.into())),
            source_pool: source_pool.clone(),
            cost_function: None,
        }
    }

    /// Sets the initial level of this upgrade and returns self.
    #[must_use]
    pub fn with_level(self, initial_level: impl Into<Approximint>) -> Self {
        self.level.set(initial_level.into());
        self
    }

    /// Returns the pool containing the number of times this upgrade has been
    /// purchased.
    #[must_use]
    pub const fn level(&self) -> &ResourcePool {
        &self.level
    }

    /// Returns a dynamic containing the current cost of the upgrade.
    ///
    /// If `None`, the upgrade is not able to be purchased.
    #[must_use]
    pub const fn cost(&self) -> &Dynamic<Option<Approximint>> {
        &self.cost
    }

    /// Returns the pool that this upgrade purchases from.
    #[must_use]
    pub const fn source_pool(&self) -> &ResourcePool {
        &self.source_pool
    }

    /// Applies `cost_fn` each time the upgrade is purchased.
    ///
    /// `cost_fn` is provided two parameters:
    ///
    /// - The current upgrade level
    /// - The current cost
    ///
    /// `cost_fn` returns the cost to purchase the next upgrade level as an
    /// `Option<Approxmint>`. If `None` is returned, upgrade purchasing is
    /// disabled.
    #[must_use]
    pub fn with_cost_fn<CostFn>(mut self, mut cost_fn: CostFn) -> Self
    where
        CostFn: FnMut(Approximint, Approximint) -> Option<Approximint> + Send + 'static,
    {
        self.cost_function = Some(SharedCallback::new(move |(level, cost)| {
            cost_fn(level, cost)
        }));
        self
    }

    /// Returns a button with the given caption that purchases this upgrade.
    #[must_use]
    pub fn purchase_button_with_caption(&self, caption: impl MakeWidget) -> WidgetInstance {
        let source = self.source_pool.clone();
        let cost = self.cost.clone();
        let enabled = (&*source, &cost)
            .map_each(|(source, cost)| cost.as_ref().map_or(false, |cost| cost <= source));
        caption
            .into_button()
            .on_click({
                let level = self.level.clone();
                let cost_fn = self.cost_function.clone();
                move |_| {
                    let current_cost = cost.get();
                    if let Some(current_cost) = current_cost {
                        let mut source = source.lock();
                        if current_cost <= *source {
                            *source -= current_cost;
                            drop(source);

                            let mut level = level.lock();
                            *level += Approximint::ONE;
                            if let Some(cost_fn) = &cost_fn {
                                let new_cost = cost_fn.invoke((*level, current_cost));
                                drop(level);
                                cost.set(new_cost);
                            }
                        }
                    }
                }
            })
            .with_enabled(enabled)
    }

    /// Returns a button that purchases this upgrade with a caption produced by
    /// invoking `caption` when this upgrade or its cost changes.
    ///
    /// `caption` accepts two parameters:
    ///
    /// - The current upgrade level
    /// - The current cost
    pub fn purchase_button(
        &self,
        caption: impl Fn(Approximint, Option<Approximint>) -> String + Send + 'static,
    ) -> WidgetInstance {
        let caption =
            (&*self.level, &self.cost).map_each_cloned(move |(level, cost)| caption(level, cost));
        self.purchase_button_with_caption(caption)
    }

    /// Returns a button that purchases this upgrade with a caption produced by
    /// invoking `caption` when this upgrade, `quantity`, or its cost changes.
    ///
    /// `caption` accepts three parameters:
    ///
    /// - The current upgrade level
    /// - The current `quantity`
    /// - The current cost
    pub fn purchase_button_with_quantity(
        &self,
        quantity: &ResourcePool,
        caption: impl Fn(Approximint, Approximint, Option<Approximint>) -> String + Send + 'static,
    ) -> WidgetInstance {
        let caption = (&*self.level, &**quantity, &self.cost)
            .map_each_cloned(move |(level, quantity, cost)| caption(level, quantity, cost));
        self.purchase_button_with_caption(caption)
    }
}
