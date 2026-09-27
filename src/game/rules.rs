use crate::error::AppError;
use crate::game::command::GameCommand;
use crate::game::event::{GameEvent, GameEventData, ScheduledEvent};
use crate::game::game::{Player, PlayerId};
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
    pub base_build_ticks: u64,
}

pub const fn spec(building: Building) -> BuildingSpec {
    match building {
        Building::Sawmill => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(100, 0, 0, 0),
            base_output: Resources::new(10, 0, 0, 0),
            base_build_ticks: 100,
        },
        Building::ClayPit => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(1000, 0, 0, 0),
            base_output: Resources::new(0, 10, 0, 0),
            base_build_ticks: 100,
        },
        Building::Mine => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 500, 0, 0),
            base_output: Resources::new(10, 0, 10, 0),
            base_build_ticks: 100,
        },
        Building::Farm => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 500, 0, 0),
            base_output: Resources::new(10, 0, 0, 10),
            base_build_ticks: 100,
        },
        Building::Warehouse => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(1000, 0, 0, 0),
            base_output: Resources::new(0, 0, 0, 0),
            base_build_ticks: 100,
        },
        Building::Barracks => BuildingSpec {
            max_level: 10,
            base_cost: Resources::new(500, 0, 1000, 0),
            base_output: Resources::new(0, 0, 0, 0),
            base_build_ticks: 100,
        },
    }
}

pub fn building_production(building: Building, level: u8) -> Resources {
    spec(building).base_output.multiply(level.into())
}

pub fn production(settlement: &Settlement) -> Resources {
    settlement
        .building_levels
        .iter()
        .fold(Resources::default(), |total, (&building, &level)| {
            total.add(building_production(building, level))
        })
}
/// Scales the base cost by Fibonacci multipliers: 1, 1, 2, 3, 5, ...
pub fn upgrade_cost(building: Building, from_level: u8) -> Resources {
    match from_level {
        0 | 1 => spec(building).base_cost,
        _ => upgrade_cost(building, from_level - 1).add(upgrade_cost(building, from_level - 2)),
    }
}

pub fn upgrade_building(
    settlement: &mut Settlement,
    building: Building,
    now_tick: u64,
) -> Result<ScheduledEvent, AppError> {
    let currentLevel = settlement.building_levels.get(building)?;

    if currentLevel == spec(building).max_level {
        return Err(AppError::InvalidCommand {
            message: format!(
                "Building reached max level {}/{}",
                currentLevel,
                spec(building).max_level
            ),
        });
    }

    settlement
        .stock
        .subtract(upgrade_cost(building, currentLevel));

    Ok(ScheduledEvent {
        data: GameEventData::BuildingUpgradeCompleted {
            settlement_id: settlement.id,
            building,
            to_level: currentLevel.saturating_add(1),
        },
        due_tick: now_tick.saturating_add(spec(building).base_build_ticks),
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

fn settlement_mut(
    state: &mut GameState,
    settlement_id: SettlementId,
) -> Result<&mut Settlement, AppError> {
    state
        .settlements
        .iter_mut()
        .find(|s| s.id == settlement_id)
        .ok_or_else(|| AppError::InvalidState {
            message: format!("Settlement {settlement_id:?} does not exist"),
        })
}

pub(crate) fn process_event(state: &mut GameState, event: GameEvent) -> Result<(), AppError> {
    match event.data {
        GameEventData::BuildingUpgradeCompleted {
            settlement_id,
            building,
            to_level,
        } => {
            settlement_mut(state, settlement_id)?
                .building_levels
                .insert(building, to_level);
        }
    }

    Ok(())
}

pub(crate) fn execute_command(
    state: &mut GameState,
    command: GameCommand,
) -> Result<(Vec<ScheduledEvent>), AppError> {
    match command {
        GameCommand::UpgradeBuilding {
            building,
            settlement_id,
        } => settlement_mut(state, settlement_id)
            .and_then(|settlement| upgrade_building(settlement, building, state.tick))
            .and_then(|e| Ok(vec![e])),
    }
}
