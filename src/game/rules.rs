use crate::error::AppError;
use crate::game::game::PlayerId;
use crate::game::game_state::{
    Building, GameState, PlayerState, Resources, Settlement, SettlementId,
};
use std::collections::BTreeMap;
use time::OffsetDateTime;

pub const TICK_SECONDS: u64 = 60;
pub const SETTLEMENT_NAMES: &[&str] = &[
    "Paris",
    "London",
    "Athens",
    "Rome",
    "Lisbon",
    "Prague",
    "Marseille",
    "Naples",
    "Dubrovnik",
    "Belgrade",
    "York",
];

pub fn tick_at(created_at: OffsetDateTime, now: OffsetDateTime) -> u64 {
    (now - created_at).whole_seconds().max(0) as u64 / TICK_SECONDS
}

/// Static content. Never serialized, never migrated, never in GameState.
pub struct BuildingSpec {
    pub max_level: u8,
    pub base_cost: Resources,
    pub base_output: Resources,
}

pub const fn spec(building: Building) -> BuildingSpec {
    match building {
        Building::Sawmill => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(100, 0, 0, 0),
            base_output: Resources::new(10, 0, 0, 0),
        },
        Building::ClayPit => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(1000, 0, 0, 0),
            base_output: Resources::new(0, 10, 0, 0),
        },
        Building::Mine => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 500, 0, 0),
            base_output: Resources::new(10, 0, 10, 0),
        },
        Building::Farm => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 500, 0, 0),
            base_output: Resources::new(10, 0, 0, 10),
        },
        Building::Warehouse => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(1000, 0, 0, 0),
            base_output: Resources::new(0, 0, 0, 0),
        },
        Building::Barracks => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 0, 1000, 0),
            base_output: Resources::new(0, 0, 0, 0),
        },
    }
}

pub fn building_production(building: Building, level: u8) -> Resources {
    spec(building).base_output.multiply(level.into())
}

pub fn upgrade_cost(building: Building, from_level: u8) -> Resources {
    todo!()
}
pub fn production(settlement: &Settlement) -> Resources {
    settlement
        .building_levels
        .iter()
        .fold(Resources::default(), |total, (&building, &level)| {
            total.add(building_production(building, level))
        })
}

pub fn storage_capacity(state: &GameState, settlement: SettlementId) -> i64 {
    todo!()
}
pub fn new_settlement(settlements_state: &Vec<Settlement>, owner: PlayerId) -> Settlement {
    let settlement_count = settlements_state.len();
    Settlement {
        id: SettlementId::from_usize(settlement_count),
        owner,
        name: SETTLEMENT_NAMES[settlement_count].to_owned(),
        stock: Resources::new(200, 200, 200, 200),
        building_levels: BTreeMap::new(),
        construction_queue: Vec::new(),
    }
}

pub(crate) fn initial_state(players: &[PlayerId]) -> Result<GameState, AppError> {
    let mut settlements = Vec::new();
    for &player_id in players {
        settlements.push(new_settlement(&settlements, player_id));
    }

    let game_state = GameState {
        tick: 0,
        players: players
            .iter()
            .copied()
            .map(|id| (id, PlayerState { score: 0 }))
            .collect(),
        settlements,
    };

    Ok(game_state)
}

pub(crate) fn advance_to(state: &mut GameState, now: u64) {
    let Some(elapsed) = now.checked_sub(state.tick) else {
        return;
    };
    if elapsed == 0 {
        return;
    }

    for settlement in &mut state.settlements {
        settlement.stock = settlement
            .stock
            .add(production(settlement).multiply(elapsed))
    }

    state.tick = now;
}
