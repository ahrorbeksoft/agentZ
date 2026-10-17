use std::time::Duration;

use gpui::{Animation, AnimationElement, AnimationExt, Transformation, percentage};

use crate::{prelude::*, traits::transformable::Transformable};

/// How often an animation that goes on while something works (a spinner, a pulse, a shimmer)
/// redraws. Each redraw lays out and paints the whole window (no view is cached, so each one
/// renders again), so redrawing on every frame, 60 times a second, kept the main thread at
/// 35-45% of a core for as long as a turn ran. A spinner or a shimmer still reads as smooth at
/// 15.
pub const LOOPING_ANIMATION_FPS: f32 = 15.;

/// An extension trait for adding common animations to animatable components.
pub trait CommonAnimationExt: AnimationExt {
    /// Render this component as rotating over the given duration.
    ///
    /// NOTE: This method uses the location of the caller to generate an ID for this state.
    ///       If this is not sufficient to identify your state (e.g. you're rendering a list item),
    ///       you can provide a custom ElementID using the `use_keyed_rotate_animation` method.
    #[track_caller]
    fn with_rotate_animation(self, duration: u64) -> AnimationElement<Self>
    where
        Self: Transformable + Sized,
    {
        self.with_keyed_rotate_animation(
            ElementId::CodeLocation(*std::panic::Location::caller()),
            duration,
        )
    }

    /// Render this component as rotating with the given element ID over the given duration.
    fn with_keyed_rotate_animation(
        self,
        id: impl Into<ElementId>,
        duration: u64,
    ) -> AnimationElement<Self>
    where
        Self: Transformable + Sized,
    {
        self.with_animation(
            id,
            Animation::new(Duration::from_secs(duration))
                .repeat_synced()
                .with_max_fps(LOOPING_ANIMATION_FPS),
            |component, delta| component.transform(Transformation::rotate(percentage(delta))),
        )
    }
}

impl<T: AnimationExt> CommonAnimationExt for T {}
