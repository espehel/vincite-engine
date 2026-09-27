use crate::game::game_state::{Building, SettlementId};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub(crate) enum GameCommand {
    UpgradeBuilding {
        settlement_id: SettlementId,
        building: Building,
    },
}
