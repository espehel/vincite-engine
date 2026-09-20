use crate::game::game::PlayerId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Building {
    Sawmill,
    ClayPit,
    Mine,
    Farm,
    Warehouse,
    Barracks,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Construction {
    pub building: Building,
    pub to_level: u8,
    pub completes_at_tick: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Wood,
    Clay,
    Iron,
    Grain,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Resources {
    #[serde(default)]
    pub wood: i64,
    #[serde(default)]
    pub clay: i64,
    #[serde(default)]
    pub iron: i64,
    #[serde(default)]
    pub grain: i64,
}

impl Resources {
    pub const fn new(wood: i64, clay: i64, iron: i64, grain: i64) -> Self {
        Self {
            wood,
            clay,
            iron,
            grain,
        }
    }
    pub fn multiply(self, factor: u64) -> Self {
        let factor = i64::try_from(factor).unwrap_or(i64::MAX);
        Self {
            wood: self.wood.saturating_mul(factor),
            clay: self.clay.saturating_mul(factor),
            iron: self.iron.saturating_mul(factor),
            grain: self.grain.saturating_mul(factor),
        }
    }
    pub fn add(self, other: Self) -> Self {
        Self {
            wood: self.wood.saturating_add(other.wood),
            clay: self.clay.saturating_add(other.clay),
            iron: self.iron.saturating_add(other.iron),
            grain: self.grain.saturating_add(other.grain),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(transparent)]
pub struct SettlementId(u32);
impl SettlementId {
    pub(crate) fn from_usize(id: usize) -> Self {
        Self(u32::try_from(id).expect("Settlement id exceeds u32::MAX"))
    }
    pub(crate) fn into_usize(self) -> usize {
        usize::try_from(self.0).expect("Settlement id exceeds usize::MAX")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Settlement {
    pub id: SettlementId,
    pub owner: PlayerId,
    pub name: String,
    pub stock: Resources,
    pub building_levels: BTreeMap<Building, u8>,
    pub construction_queue: Vec<Construction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerState {
    pub score: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameState {
    pub(in crate::game) tick: u64,
    pub(in crate::game) players: BTreeMap<PlayerId, PlayerState>,
    pub(in crate::game) settlements: Vec<Settlement>,
}

impl GameState {
    pub(crate) fn is_initialized(&self) -> bool {
        !self.settlements.is_empty() && self.settlements.len() >= self.players.keys().len()
    }
}
