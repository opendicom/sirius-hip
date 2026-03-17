use serde::{Deserialize, Serialize};

/// Enum representing supported database versions for the application. This allows 
/// for handling different database schemas or features based  on the version in use. 
/// The variants include:
/// - `dcm4chee2183`: Represents the dcm4chee version 2.18.3
/// - `dcm4chee440`: Represents the dcm4chee version 4.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] 
#[allow(non_camel_case_types)]
pub enum DBVersion {
    dcm4chee2183,
    dcm4chee440
}