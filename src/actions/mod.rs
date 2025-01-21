pub mod feed;
pub mod portfolio;
pub mod twitter;
pub mod utils;

use std::{borrow::Cow, str::FromStr, sync::OnceLock};

use crate::store::{LocalStore, Store, StoreMap};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use utils::get_cur_timestamp;

pub trait Action: ToString + FromStr {
    fn into_log(&self) -> ActionLog {
        ActionLog {
            timestamp_be_bytes: get_cur_timestamp().to_be_bytes(),
            action: self.to_string(),
            tee_signature: None, // TODO: Implement TEE tools
        }
    }

    fn from_log(log: &ActionLog) -> anyhow::Result<Self> {
        log.action
            .parse()
            .map_err(|_| anyhow!("Failed to parse action log"))
    }

    fn log(&self) {
        let log = self.into_log();
        if let Err(e) = AgentActionLog::get().actions.insert(log, ()) {
            tracing::error!("Failed to log action: {:?}", e);
        }
    }

    fn iter<'a>() -> impl Iterator<Item = (Self, Cow<'a, ActionLog>)> {
        AgentActionLog::get()
            .actions
            .iter()
            .filter_map(|(log, _)| Self::from_log(&log).ok().map(|action| (action, log)))
    }

    fn prompt(&self) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionLog {
    timestamp_be_bytes: [u8; 8], // For Ordering
    action: String,
    tee_signature: Option<String>,
}

impl ActionLog {
    pub fn timestamp(&self) -> u64 {
        u64::from_be_bytes(self.timestamp_be_bytes)
    }

    pub fn tee_signature(&self) -> Option<&str> {
        self.tee_signature.as_deref()
    }
}

#[derive(Clone)]
pub struct AgentActionLog {
    actions: StoreMap<ActionLog, (), LocalStore>,
}

impl AgentActionLog {
    const ACTIONS_PREFIX: &'static str = "agent_actions";

    pub fn get() -> &'static Self {
        static INSTANCE: OnceLock<AgentActionLog> = OnceLock::new();
        INSTANCE.get_or_init(Self::new)
    }

    fn new() -> Self {
        Self {
            actions: LocalStore::open_map(Self::ACTIONS_PREFIX),
        }
    }
}
