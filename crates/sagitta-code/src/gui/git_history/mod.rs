mod modal;
mod graph;
mod types;

pub use modal::GitHistoryModal;
pub use types::{CommitInfo, CommitNode};

#[cfg(test)]
mod tests;