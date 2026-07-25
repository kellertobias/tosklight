mod model;
mod planning;
mod ports;
mod replay;
mod service;

pub use model::{
    SPEED_GROUP_COUNT, SpeedBpm, SpeedBpmDelta, SpeedGroupAction, SpeedGroupApplication,
    SpeedGroupAuthorityProjection, SpeedGroupChange, SpeedGroupCommand, SpeedGroupDurability,
    SpeedGroupExpectation, SpeedGroupId, SpeedGroupOutcome, SpeedGroupPortState,
    SpeedGroupProjection, SpeedGroupResolvedAction, SpeedGroupResult, SpeedGroupSnapshot,
};
pub use ports::SpeedGroupPorts;
pub use service::SpeedGroupService;

#[cfg(test)]
mod tests;
