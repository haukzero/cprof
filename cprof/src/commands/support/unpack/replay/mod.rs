use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{ElevationError, Result};
use crate::targets::external::ConfigConflict;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(super) enum DecisionKey {
    Configuration(ConfigConflict),
    OverwriteProfile { target: String, profile: String },
    RemoveProfile { target: String, profile: String },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Replay {
    fingerprint: [u8; 32],
    decisions: Vec<(DecisionKey, bool)>,
}

/// Record answers in the parent; consume them by identity in the child.
#[derive(Default)]
pub(super) struct ReplaySession {
    decisions: BTreeMap<DecisionKey, bool>,
    expected: Option<[u8; 32]>,
}

impl ReplaySession {
    pub(super) fn new(context: Option<Replay>) -> Result<Self> {
        let Some(context) = context else {
            return Ok(Self::default());
        };
        let count = context.decisions.len();
        let decisions: BTreeMap<_, _> = context.decisions.into_iter().collect();
        if decisions.len() != count {
            return Err(ElevationError::InvalidContext("duplicate unpack decisions".into()).into());
        }
        Ok(Self {
            decisions,
            expected: Some(context.fingerprint),
        })
    }

    pub(super) fn is_replay(&self) -> bool {
        self.expected.is_some()
    }

    pub(super) fn decide(
        &mut self,
        key: DecisionKey,
        ask: impl FnOnce() -> Result<bool>,
    ) -> Result<bool> {
        if self.is_replay() {
            return self
                .decisions
                .remove(&key)
                .ok_or_else(|| ElevationError::ReplayMismatch.into());
        }
        if self.decisions.contains_key(&key) {
            return Err(ElevationError::InvalidContext("duplicate unpack decision".into()).into());
        }
        let answer = ask()?;
        self.decisions.insert(key, answer);
        Ok(answer)
    }

    pub(super) fn finish(&self, fingerprint: [u8; 32]) -> Result<()> {
        if let Some(expected) = self.expected
            && (expected != fingerprint || !self.decisions.is_empty())
        {
            return Err(ElevationError::ReplayMismatch.into());
        }
        Ok(())
    }

    pub(super) fn context(&self, fingerprint: [u8; 32]) -> Result<serde_json::Value> {
        serde_json::to_value(Replay {
            fingerprint,
            decisions: self
                .decisions
                .iter()
                .map(|(key, answer)| (key.clone(), *answer))
                .collect(),
        })
        .map_err(|error| ElevationError::InvalidContext(error.to_string()).into())
    }
}

#[cfg(test)]
mod tests;
