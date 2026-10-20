//! A project's flow (d0108, SDD §7.6): the stages a ticket walks between Backlog and Done. `model` holds the types,
//! `yaml` reads and writes flow files (STG-01), `check` holds the rules every flow must follow (STG-02), `default`
//! the shipped default flow (STG-03), `store` each project's applied flow and the ones before it (FLW-01), `engine`
//! the moves between stages (STG-06).

pub mod check;
pub mod default;
pub mod engine;
pub mod model;
pub mod store;
pub mod yaml;

pub use check::{Problem, Rule, check, check_agents};
pub use default::{default_file, default_flow};
pub use model::{
    CheckBy, Column, Delivers, Delivery, DeliveryMode, Flow, FlowAgent, FlowFile, Stage, StageKind,
    StageOption,
};
pub use store::{AppliedBy, FlowVersion, NewFlowVersion};
