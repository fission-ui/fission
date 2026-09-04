//! Typed, renderer-independent world queries over authoritative game state.

use crate::{
    Area2D, AreaHandle, GameState, ObjectGroupHandle, ObjectHandle, StableKeyValue, Touchable2D,
};

/// One repeated object returned with its durable collection identity.
#[derive(Clone, Debug)]
pub struct WorldObjectRef<'a, T> {
    pub key: StableKeyValue,
    pub value: &'a T,
}

/// Read-only query view over one authoritative game-state value.
#[derive(Clone, Copy, Debug)]
pub struct World<'a, S: GameState> {
    state: &'a S,
}

impl<'a, S: GameState> World<'a, S> {
    pub const fn new(state: &'a S) -> Self {
        Self { state }
    }

    /// Returns repeated objects whose exact supported contact shape touches
    /// the selected object. Collection indexes are never exposed as identity.
    pub fn touches<O, T>(
        &self,
        object: ObjectHandle<S, O>,
        group: ObjectGroupHandle<S, T>,
    ) -> Vec<WorldObjectRef<'a, T>>
    where
        O: Touchable2D,
        T: Touchable2D,
    {
        let shape = object.get(self.state).touch_area();
        group
            .get(self.state)
            .iter()
            .filter(|candidate| shape.touches(&candidate.touch_area()))
            .map(|candidate| WorldObjectRef {
                key: group.key(candidate),
                value: candidate,
            })
            .collect()
    }

    /// Tests two individually handled game objects for exact supported contact.
    pub fn objects_touch<A, B>(&self, first: ObjectHandle<S, A>, second: ObjectHandle<S, B>) -> bool
    where
        A: Touchable2D,
        B: Touchable2D,
    {
        first
            .get(self.state)
            .touch_area()
            .touches(&second.get(self.state).touch_area())
    }

    /// Reports whether any part of an object's contact bounds lies outside a
    /// declared bounded area. Empty contact shapes are never outside.
    pub fn outside<O, A>(&self, object: ObjectHandle<S, O>, area: AreaHandle<S, A>) -> bool
    where
        O: Touchable2D,
        A: Area2D,
    {
        object
            .get(self.state)
            .touch_area()
            .bounds()
            .is_some_and(|bounds| !area.get(self.state).bounds().contains_bounds(bounds))
    }
}

/// Adds the concise `state.world()` entrypoint to every game-state type.
pub trait GameStateWorldExt: GameState {
    fn world(&self) -> World<'_, Self>
    where
        Self: Sized,
    {
        World::new(self)
    }
}

impl<S: GameState> GameStateWorldExt for S {}

#[cfg(test)]
mod tests {
    use crate::{Bounds2D, Place, Px, Size, StableKey};

    use super::*;

    #[derive(Clone)]
    struct Object {
        id: u32,
        shape: crate::TouchArea,
    }

    impl Touchable2D for Object {
        fn touch_area(&self) -> crate::TouchArea {
            self.shape.clone()
        }
    }

    #[derive(Clone)]
    struct Area(Bounds2D);

    impl Area2D for Area {
        fn bounds(&self) -> Bounds2D {
            self.0
        }
    }

    #[derive(Clone)]
    struct State {
        player: Object,
        resources: Vec<Object>,
        area: Area,
    }

    impl GameState for State {}

    fn rect(x: f32, y: f32, width: f32, height: f32) -> crate::TouchArea {
        crate::TouchArea::rect(Bounds2D::from_top_left(
            Place::new(Px(x), Px(y)),
            Size::new(Px(width), Px(height)),
        ))
    }

    #[test]
    fn queries_use_typed_fields_and_return_durable_keys() {
        let state = State {
            player: Object {
                id: 1,
                shape: rect(0.0, 0.0, 10.0, 10.0),
            },
            resources: vec![
                Object {
                    id: 7,
                    shape: rect(10.0, 10.0, 2.0, 2.0),
                },
                Object {
                    id: 8,
                    shape: rect(40.0, 40.0, 2.0, 2.0),
                },
            ],
            area: Area(Bounds2D::from_top_left(
                Place::new(Px(-1.0), Px(-1.0)),
                Size::new(Px(20.0), Px(20.0)),
            )),
        };
        let player = ObjectHandle::generated("State.player", |state: &State| &state.player);
        let resources = ObjectGroupHandle::generated(
            "State.resources",
            |state: &State| state.resources.as_slice(),
            |item: &Object| item.id.stable_key(),
        );
        let area = AreaHandle::generated("State.area", |state: &State| &state.area);

        let hits = state.world().touches(player.clone(), resources);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, StableKeyValue::U64(7));
        assert!(!state.world().outside(player, area));
    }
}
