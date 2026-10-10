//! Small animations for the turn replay: cards flipping in, popping, and
//! shrinking away when destroyed.

use std::f32::consts::PI;

use bevy::prelude::*;

#[derive(Clone, Copy)]
pub enum AnimationKind {
    /// Grows from zero width, like a card turning over.
    FlipIn,
    /// Briefly grows and shrinks back, to draw the eye.
    Pop,
    /// Shrinks to nothing, then the entity is removed.
    ShrinkAway,
}

#[derive(Component)]
pub struct Animation {
    kind: AnimationKind,
    timer: Timer,
    /// The scale the entity has when not animating.
    base_scale: Vec3,
}

impl Animation {
    pub fn new(kind: AnimationKind, base_scale: f32) -> Self {
        let seconds = match kind {
            AnimationKind::FlipIn => 0.35,
            AnimationKind::Pop => 0.45,
            AnimationKind::ShrinkAway => 0.7,
        };
        Animation { kind, timer: Timer::from_seconds(seconds, TimerMode::Once), base_scale: Vec3::splat(base_scale) }
    }
}

/// Smooth start and end instead of moving at a constant speed.
fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn animate(time: Res<Time>, mut commands: Commands, mut animated: Query<(Entity, &mut Animation, &mut Transform)>) {
    for (entity, mut animation, mut transform) in &mut animated {
        animation.timer.tick(time.delta());
        let t = animation.timer.fraction();
        let base = animation.base_scale;
        transform.scale = match animation.kind {
            AnimationKind::FlipIn => Vec3::new(base.x * ease(t), base.y, base.z),
            AnimationKind::Pop => base * (1.0 + 0.35 * (PI * t).sin()),
            AnimationKind::ShrinkAway => base * (1.0 - ease(t)),
        };

        if animation.timer.is_finished() {
            match animation.kind {
                AnimationKind::ShrinkAway => commands.entity(entity).despawn(),
                _ => {
                    transform.scale = base;
                    commands.entity(entity).remove::<Animation>();
                }
            }
        }
    }
}
