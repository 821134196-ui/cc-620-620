use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tree {
    pub id: i64,
    pub tree_code: String,
    pub road_name: String,
    pub species: String,
    pub x: f64,
    pub y: f64,
    pub notes: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTree {
    pub tree_code: String,
    pub road_name: String,
    pub species: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Sample {
    pub id: i64,
    pub sample_code: String,
    pub tree_id: i64,
    pub round: i64,
    pub parent_sample_id: Option<i64>,
    pub status: String,
    pub inspector: String,
    pub symptom: String,
    pub return_reason: String,
    pub submitted_at: String,
    pub returned_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitSample {
    pub tree_id: Option<i64>,
    pub inspector: String,
    pub symptom: String,
    /// 退样重送时指向原样本；缺省为沿树号新开一轮送样
    pub parent_sample_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ReturnSample {
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LabResult {
    pub id: i64,
    pub sample_id: i64,
    pub conclusion: String, // pest_confirmed / healthy / undetermined
    pub diagnosis: String,
    pub severity: String,
    pub note: String,
    pub is_draft: i64,
    pub recorded_by: String,
    pub verified_by: String,
    pub created_at: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordResult {
    pub conclusion: String,
    #[serde(default)]
    pub diagnosis: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub note: String,
    pub recorded_by: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyResult {
    pub verified_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Treatment {
    pub id: i64,
    pub treatment_code: String,
    pub tree_id: i64,
    pub sample_id: i64,
    pub round: i64,
    pub assigned_unit: String,
    pub status: String, // issued / in_progress / completed
    pub report_summary: String,
    pub issued_at: String,
    pub accepted_at: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IssueTreatment {
    pub sample_id: i64,
    pub assigned_unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TreatmentReport {
    pub id: i64,
    pub treatment_id: i64,
    pub content: String,
    pub reporter: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AddReport {
    pub content: String,
    pub reporter: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Notification {
    pub id: i64,
    pub channel: String,
    pub target: String,
    pub title: String,
    pub body: String,
    pub related_tree_id: Option<i64>,
    pub public_visible: i64,
    pub created_at: String,
}

/// 工作人员端：样本 + 鉴定结果（含草稿）+ 关联处置单
#[derive(Debug, Serialize)]
pub struct SampleFull {
    pub sample: Sample,
    pub parent_sample_code: Option<String>,
    pub lab_result: Option<LabResult>,
    pub treatment: Option<TreatmentWithReports>,
}

#[derive(Debug, Serialize)]
pub struct TreatmentWithReports {
    pub treatment: Treatment,
    pub reports: Vec<TreatmentReport>,
}

#[derive(Debug, Serialize)]
pub struct TreeHistory {
    pub tree: Tree,
    pub samples: Vec<SampleFull>,
}

/// 居民公开页时间线条目（仅含已核实内容）
#[derive(Debug, Serialize)]
pub struct PublicTimelineItem {
    pub kind: String, // sample_verified / treatment_status
    pub at: String,
    pub round: i64,
    pub sample_code: String,
    pub parent_sample_code: Option<String>,
    pub conclusion: String,
    pub diagnosis: String,
    pub severity: String,
    pub note: String,
    pub treatment_code: Option<String>,
    pub treatment_status: Option<String>,
    pub assigned_unit: Option<String>,
    pub report_summary: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TreePublic {
    pub tree: Tree,
    /// 依据最近一次“已核实”结论给出的公开状态
    pub public_status: String,
    pub public_status_text: String,
}

#[derive(Debug, Serialize)]
pub struct TreePublicDetail {
    pub tree: Tree,
    pub public_status: String,
    pub public_status_text: String,
    pub timeline: Vec<PublicTimelineItem>,
}
