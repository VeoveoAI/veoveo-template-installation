//! `veoveo-installer` makes a cluster match this installation repository. It is an
//! operator's command, not a service: it keeps no state of its own and runs only when
//! an operator runs it. `plan` compares each declared piece with what runs, through that
//! piece's owner, and changes nothing.

pub mod action;
pub mod apply;
pub mod declaration;
pub mod difference;
pub mod helm;
pub mod hub;
pub mod inputs;
pub mod kubernetes;
pub mod openshell;
pub mod plan;
pub mod private;
pub mod run;
pub mod yaml;
