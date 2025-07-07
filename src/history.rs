use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("Failed to create history directory: {0}")]
    CreateDir(#[from] std::io::Error),

    #[error("Failed to serialize/deserialize history: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Failed to find home directory")]
    NoHomeDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub job_id: String,
    pub class_hash: String,
    pub contract_name: String,
    pub network: String,
    pub timestamp: DateTime<Utc>,
    pub status: Option<String>,
    pub project_path: Option<String>,
    pub license: Option<String>,
}

impl VerificationRecord {
    pub fn new(
        job_id: String,
        class_hash: String,
        contract_name: String,
        network: String,
        project_path: Option<String>,
        license: Option<String>,
    ) -> Self {
        Self {
            job_id,
            class_hash,
            contract_name,
            network,
            timestamp: Utc::now(),
            status: None,
            project_path,
            license,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationHistory {
    pub records: HashMap<String, VerificationRecord>,
}

impl VerificationHistory {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
        }
    }

    pub fn add_record(&mut self, record: VerificationRecord) {
        self.records.insert(record.job_id.clone(), record);
    }

    pub fn update_status(&mut self, job_id: &str, status: String) {
        if let Some(record) = self.records.get_mut(job_id) {
            record.status = Some(status);
        }
    }

    pub fn get_recent_records(&self, limit: usize) -> Vec<&VerificationRecord> {
        let mut records: Vec<&VerificationRecord> = self.records.values().collect();
        records.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        records.into_iter().take(limit).collect()
    }
}

pub struct HistoryManager {
    history_file: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Result<Self, HistoryError> {
        let home_dir = dirs::home_dir().ok_or(HistoryError::NoHomeDir)?;
        let history_dir = home_dir.join(".starknet-verifier");
        let history_file = history_dir.join("history.json");

        if !history_dir.exists() {
            fs::create_dir_all(&history_dir)?;
        }

        Ok(Self { history_file })
    }

    pub fn load_history(&self) -> Result<VerificationHistory, HistoryError> {
        if !self.history_file.exists() {
            return Ok(VerificationHistory::new());
        }

        let content = fs::read_to_string(&self.history_file)?;
        let history: VerificationHistory = serde_json::from_str(&content)?;
        Ok(history)
    }

    pub fn save_history(&self, history: &VerificationHistory) -> Result<(), HistoryError> {
        let content = serde_json::to_string_pretty(history)?;
        fs::write(&self.history_file, content)?;
        Ok(())
    }

    pub fn add_verification(&self, record: VerificationRecord) -> Result<(), HistoryError> {
        let mut history = self.load_history()?;
        history.add_record(record);
        self.save_history(&history)?;
        Ok(())
    }

    pub fn update_verification_status(
        &self,
        job_id: &str,
        status: String,
    ) -> Result<(), HistoryError> {
        let mut history = self.load_history()?;
        history.update_status(job_id, status);
        self.save_history(&history)?;
        Ok(())
    }

    pub fn list_recent_jobs(&self, limit: usize) -> Result<Vec<VerificationRecord>, HistoryError> {
        let history = self.load_history()?;
        Ok(history
            .get_recent_records(limit)
            .into_iter()
            .cloned()
            .collect())
    }
}
