//! Vendor-neutral corpus manifest schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusManifest {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub sources: Vec<ManifestSource>,
    #[serde(default)]
    pub artifacts: Vec<ManifestArtifact>,
    #[serde(default)]
    pub code_references: Vec<ManifestCodeReference>,
    #[serde(default)]
    pub relationships: Vec<ManifestRelationship>,
    #[serde(default)]
    pub expected_relationships: Vec<ManifestRelationship>,
    #[serde(default)]
    pub external_analyses: Vec<ManifestExternalAnalysis>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestSource {
    pub id: String,
    pub kind: String,
    pub origin: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestArtifact {
    pub id: String,
    pub source: String,
    pub path: String,
    pub title: String,
    pub document_type: String,
    pub media_type: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestCodeReference {
    pub id: String,
    pub repository: String,
    pub revision: String,
    pub path: String,
    pub symbol: Option<String>,
    pub line_start: Option<u64>,
    pub line_end: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestRelationship {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub origin: String,
    pub verification: String,
    pub external_analysis: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestExternalAnalysis {
    pub id: String,
    pub provider: String,
    pub provider_version: String,
    pub run_id: String,
    pub configuration_digest: String,
    pub observed_at: Option<String>,
}
