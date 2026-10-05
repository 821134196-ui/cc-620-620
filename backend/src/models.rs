use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Tree {
    pub id: i64,
    pub tree_no: String,
    pub species: String,
    pub road: String,
    pub address: String,
    pub lat: f64,
    pub lng: f64,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Round {
    pub id: i64,
    pub tree_id: i64,
    pub round_no: i64,
    pub status: String,
    pub reason: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Sample {
    pub id: i64,
    pub sample_no: String,
    pub tree_id: i64,
    pub round_id: i64,
    pub parent_sample_id: Option<i64>,
    pub inspector: String,
    pub symptom: String,
    pub note: String,
    pub status: String,
    pub submitted_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SampleResult {
    pub id: i64,
    pub sample_id: i64,
    pub result_type: String,
    pub pest_name: String,
    pub confidence: String,
    pub notes: String,
    pub status: String,
    pub created_by: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Disposal {
    pub id: i64,
    pub disposal_no: String,
    pub round_id: i64,
    pub tree_id: i64,
    pub sample_id: Option<i64>,
    pub assignee: String,
    pub method: String,
    pub status: String,
    pub issued_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TreatmentReport {
    pub id: i64,
    pub disposal_id: i64,
    pub reporter: String,
    pub content: String,
    pub reported_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: i64,
    pub recipient: String,
    pub channel: String,
    pub title: String,
    pub body: String,
    pub ref_type: String,
    pub ref_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MapRoad {
    pub id: i64,
    pub name: String,
    pub path: String,
}

// ---------- 请求体 ----------

#[derive(Debug, Deserialize)]
pub struct CreateTree {
    pub tree_no: Option<String>,
    pub species: String,
    pub road: String,
    pub address: Option<String>,
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Deserialize)]
pub struct CreateSample {
    pub tree_id: i64,
    pub inspector: String,
    pub symptom: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordResult {
    pub result_type: String, // CONFIRMED / NEGATIVE / INCONCLUSIVE
    pub pest_name: Option<String>,
    pub confidence: Option<String>,
    pub notes: Option<String>,
    pub created_by: String,
}

#[derive(Debug, Deserialize)]
pub struct RejectSample {
    pub reason: String,
    pub created_by: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateDisposal {
    pub round_id: i64,
    pub sample_id: Option<i64>,
    pub assignee: String,
    pub method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReport {
    pub reporter: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenRound {
    pub reason: Option<String>,
}
