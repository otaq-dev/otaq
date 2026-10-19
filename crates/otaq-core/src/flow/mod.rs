//! A project's flow (d0108, SDD §7.6): the stages a ticket walks between Backlog and Done. `model` holds the types,
//! `yaml` reads and writes flow files (STG-01), `check` holds the rules every flow must follow (STG-02), `default`
//! the shipped default flow and the flow a project derives from it (STG-03).

pub mod check;
pub mod default;
pub mod model;
pub mod yaml;

pub use check::{Problem, Rule, check};
pub use default::{default_file, default_flow, effective_flow};
pub use model::{
    CheckBy, Column, Delivers, Delivery, DeliveryMode, Flow, FlowAgent, FlowFile, Stage, StageKind,
    StageOption,
};
