use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DocWorkspaceResponse {
    pub workspace_root: String,
    pub workspace_manifest_path: String,
    pub target_directory: String,
    pub package_count: usize,
    pub packages: Vec<DocPackageSummary>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DocPackageSummary {
    pub name: String,
    pub version: String,
    pub package_root: String,
    pub target_count: usize,
    pub doc_target_count: usize,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ArtifactListResponse {
    pub workspace_root: String,
    pub target_directory: String,
    pub artifact_count: usize,
    pub artifacts: Vec<CargoDocArtifact>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CargoDocArtifact {
    pub package_name: String,
    pub package_root: String,
    pub target_name: String,
    pub target_kind: Vec<String>,
    pub html_root_dir: String,
    pub html_index_path: String,
    pub html_exists: bool,
    pub rustdoc_json_path: String,
    pub rustdoc_json_exists: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DocIndexResponse {
    pub workspace: DocWorkspaceResponse,
    pub artifacts: Vec<CargoDocArtifact>,
}