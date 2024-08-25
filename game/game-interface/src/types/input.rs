use std::{
    marker::PhantomData,
    num::{NonZeroI64, NonZeroU64},
    ops::{AddAssign, Deref},
};

use either::Either;
use hiarc::Hiarc;
use math::math::vector::dvec2;
use serde::{de, ser, Deserialize, Serialize};

use super::weapons::WeaponType;

/// the character cursor has two guarantees:
/// - x and y are never NaN or infinite
/// - x and y are never both 0 at the same time
///     (they have a threshold so that normalizing always works)
#[derive(Debug, Hiarc, Copy, Clone, PartialEq)]
pub struct CharacterInputCursor {
    x: f64,
    y: f64,
}

impl<'de> de::Deserialize<'de> for CharacterInputCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        <dvec2 as de::Deserialize>::deserialize(deserializer).and_then(|inner| {
            if !inner.x.is_finite() || !inner.y.is_finite() {
                Err(de::Error::invalid_value(
                    de::Unexpected::Float(if !inner.x.is_finite() {
                        inner.x
                    } else {
                        inner.y
                    }),
                    &"the value of either x or y was NaN or infinite",
                ))
            } else if inner.x.abs() < Self::MIN_CURSOR_VAL && inner.y.abs() < Self::MIN_CURSOR_VAL {
                Err(de::Error::invalid_value(
                    de::Unexpected::Float(if inner.x.abs() < Self::MIN_CURSOR_VAL {
                        inner.x
                    } else {
                        inner.y
                    }),
                    &"the value of either x or y must not be under the threshold of 0.0001",
                ))
            } else {
                Ok(Self {
                    x: inner.x,
                    y: inner.y,
                })
            }
        })
    }
}

impl Serialize for CharacterInputCursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        <dvec2 as ser::Serialize>::serialize(&dvec2::new(self.x, self.y), serializer)
    }
}

impl Default for CharacterInputCursor {
    fn default() -> Self {
        Self {
            x: Self::MIN_CURSOR_VAL,
            y: Default::default(),
        }
    }
}

impl CharacterInputCursor {
    pub const MIN_CURSOR_VAL: f64 = 0.0001;

    pub fn to_vec2(&self) -> dvec2 {
        dvec2::new(self.x, self.y)
    }
    pub fn from_vec2(cursor: &dvec2) -> Self {
        // make sure 0,0 is prevented
        let mut cursor = *cursor;
        if !cursor.x.is_finite() || !cursor.y.is_finite() {
            // reset broken coordinate
            cursor = dvec2::new(1.0, 0.0);
        } else if cursor.x.abs() < Self::MIN_CURSOR_VAL && cursor.y.abs() < Self::MIN_CURSOR_VAL {
            cursor.x = Self::MIN_CURSOR_VAL;
        }
        Self {
            x: cursor.x,
            y: cursor.y,
        }
    }
}

#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputVarConsumable<V> {
    val: V,
}

impl<V: PartialEq + AddAssign<V>> InputVarConsumable<V> {
    pub fn add(&mut self, val: V) {
        self.val += val;
    }
}

/// Some input is positioned by a cursor
#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PositionedInputVarConsumable<V> {
    val: InputVarConsumable<V>,
    cursor: CharacterInputCursor,
}

impl<V: PartialEq + AddAssign<V>> PositionedInputVarConsumable<V> {
    pub fn add(&mut self, val: V, at_cursor: CharacterInputCursor) {
        self.val.add(val);
        self.cursor = at_cursor;
    }
}

#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputVarState<V> {
    val: V,
}

impl<V: PartialEq> InputVarState<V> {
    pub fn set(&mut self, val: V) {
        if val != self.val {
            self.val = val;
        }
    }
}

impl<V> Deref for InputVarState<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

#[derive(Debug, Hiarc, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterInputConsumableDiff {
    pub jump: Option<NonZeroU64>,
    pub fire: Option<(NonZeroU64, CharacterInputCursor)>,
    pub hook: Option<(NonZeroU64, CharacterInputCursor)>,
    pub weapon_req: Option<WeaponType>,
    pub weapon_diff: Option<NonZeroI64>,

    // don't allow contructing outside of this file
    _prevent: PhantomData<()>,
}

/// To get const size for the weapon request,
/// use a wrapper that serializes it to the "same"
/// thing.
#[derive(Debug, Hiarc, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WeaponReq(pub Option<WeaponType>);

impl Serialize for WeaponReq {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let val = match self.0 {
            Some(val) => Either::Right(val),
            None => Either::Left(WeaponType::default()),
        };

        val.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WeaponReq {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match <Either<WeaponType, WeaponType>>::deserialize(deserializer) {
            Ok(val) => Ok(match val {
                Either::Left(_) => Self(None),
                Either::Right(val) => Self(Some(val)),
            }),
            Err(err) => Err(err),
        }
    }
}

impl Deref for WeaponReq {
    type Target = Option<WeaponType>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Option<WeaponType>> for WeaponReq {
    fn from(value: Option<WeaponType>) -> Self {
        Self(value)
    }
}

#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterInputConsumable {
    pub jump: InputVarConsumable<u64>,
    pub fire: PositionedInputVarConsumable<u64>,
    pub hook: PositionedInputVarConsumable<u64>,
    weapon_req: InputVarState<WeaponReq>,
    pub weapon_diff: InputVarConsumable<i64>,
}

impl CharacterInputConsumable {
    /// Create the difference between two consumable input states.
    /// The difference means, the amount of clicks that happened etc.
    pub fn diff(&self, other: &Self) -> CharacterInputConsumableDiff {
        let jump = self.jump.val.saturating_sub(other.jump.val);
        let fire = self.fire.val.val.saturating_sub(other.fire.val.val);
        let hook = self.hook.val.val.saturating_sub(other.hook.val.val);
        let weapon_req = self.weapon_req.val != other.weapon_req.val;
        let weapon_diff = self.weapon_diff.val.saturating_sub(other.weapon_diff.val);

        CharacterInputConsumableDiff {
            jump: if jump == 0 {
                None
            } else {
                Some(NonZeroU64::new(jump).unwrap())
            },
            fire: if fire == 0 {
                None
            } else {
                Some((NonZeroU64::new(fire).unwrap(), self.fire.cursor))
            },
            hook: if hook == 0 {
                None
            } else {
                Some((NonZeroU64::new(hook).unwrap(), self.hook.cursor))
            },
            weapon_req: weapon_req.then_some(*self.weapon_req.val).flatten(),
            weapon_diff: if weapon_diff == 0 {
                None
            } else {
                Some(NonZeroI64::new(weapon_diff).unwrap())
            },

            _prevent: Default::default(),
        }
    }

    pub fn set_weapon_req(&mut self, val: Option<WeaponType>) {
        self.weapon_req.set(val.into())
    }

    /// weapon diff also needs special treatment to prevent sending too much input.
    pub fn only_weapon_diff_changed(&mut self, other: &Self) -> bool {
        let diff = self.diff(other);

        diff.jump.is_none()
            && diff.fire.is_none()
            && diff.hook.is_none()
            && diff.weapon_req.is_none()
            && diff.weapon_diff.is_some()
    }
}

#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterInputState {
    pub dir: InputVarState<i32>,
    pub hook: InputVarState<bool>,
    pub fire: InputVarState<bool>,
    pub jump: InputVarState<bool>,
}

/// character input splits into two categories:
/// - consumable input: these inputs are private and can only be queried by
///     comparing it to another input. They represent an input event
///     (was fired, has jumped, was weapon changed etc.)
/// - stateful input: these inputs are like a current state of the input and
///     can be queried all the time (current cursor, hold hook button, hold fire button etc.)
#[derive(Debug, Hiarc, Copy, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterInput {
    pub cursor: InputVarState<CharacterInputCursor>,

    pub state: CharacterInputState,
    pub consumable: CharacterInputConsumable,
}
