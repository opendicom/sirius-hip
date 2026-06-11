use async_trait::async_trait;
use reqwest::{Client, Url};
use serde_json::Value;

use crate::{
    features::study_token::entities::Study,
    pacs::{
        Instance, InstanceSearchCriteria, MetadataProvider, PacsConnectorError, Series, SeriesSearchCriteria, StudySearchCriteria
    },
};

#[derive(Clone)]
pub struct Dcm4chee2183DicomWebMetadataProvider {
    pacs_id: String,
    _http_client: Client,
    _base_url: String,
}

impl Dcm4chee2183DicomWebMetadataProvider {
    pub fn new(pacs_id: String, http_client: Client, base_url: String) -> Self {
        Self {
            pacs_id,
            _http_client: http_client,
            _base_url: base_url,
        }
    }

    fn _build_qido_studies_url(&self, criteria: &StudySearchCriteria) -> Result<Url, PacsConnectorError> {
        let mut url = Url::parse(&self._base_url).map_err(|_| PacsConnectorError::InvalidDicomwebBaseUrl {
            pacs_id: self.pacs_id.clone(),
            base_url: self._base_url.clone(),
        })?;

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| PacsConnectorError::InvalidDicomwebBaseUrl {
                    pacs_id: self.pacs_id.clone(),
                    base_url: self._base_url.clone(),
                })?;
            segments.push("studies");
        }

        {
            let mut query = url.query_pairs_mut();

            if let Some(patient_id) = criteria.patient_id.as_deref() {
                query.append_pair("PatientID", patient_id);
            }

            if let Some(accession_number) = criteria.accession_number.as_deref() {
                query.append_pair("AccessionNumber", accession_number);
            }
        }

        Ok(url)
    }
}


fn _dicom_json_first_string(row: &Value, tag: &str) -> Option<String> {
    row.get(tag)
        .and_then(|node| node.get("Value"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.as_str().map(ToString::to_string))
}

fn _dicom_json_patient_name(row: &Value) -> Option<String> {
    row.get("00100010")
        .and_then(|node| node.get("Value"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.get("Alphabetic"))
        .and_then(|alphabetic| alphabetic.as_str().map(ToString::to_string))
        .or_else(|| _dicom_json_first_string(row, "00100010"))
}


#[async_trait]
impl MetadataProvider for Dcm4chee2183DicomWebMetadataProvider {

    async fn require_dirty_triggers(&self) -> Result<(), PacsConnectorError> {
        Err(PacsConnectorError::UnsupportedOperation {
            pacs_id: "unknown".to_string(),
            operation: "require_dirty_triggers",
            reason: "dcm4chee2183 + dicomweb metadata provider does not support dirty triggers",
        })
    }

    async fn search_studies(&self, _criteria: &StudySearchCriteria) -> Result<Vec<Study>, PacsConnectorError> {
        // let url = self.build_qido_studies_url(criteria)?;

        // let response = self
        //     .http_client
        //     .get(url)
        //     .header("Accept", "application/dicom+json")
        //     .send()
        //     .await?
        //     .error_for_status()?;

        // let body = response.bytes().await?;
        // let rows: Vec<Value> = serde_json::from_slice(&body)?;

        // Ok(rows
        //     .into_iter()
        //     .filter_map(|row| {
        //         let study_uid = dicom_json_first_string(&row, "0020000D")?;

        //         Some(Study {
        //             study_uid,
        //             patient_id: dicom_json_first_string(&row, "00100020").unwrap_or_default(),
        //             patient_name: dicom_json_patient_name(&row).unwrap_or_default(),
        //             accession_number: dicom_json_first_string(&row, "00080050"),
        //         })
        //     })
        //     .collect())
        Err(PacsConnectorError::UnsupportedOperation {
            pacs_id: "unknown".to_string(),
            operation: "search_studies",
            reason: "dcm4chee2183 + dicomweb metadata provider does not support study search yet",
        })

    }

    async fn search_series(&self, _criteria: &SeriesSearchCriteria) -> Result<Vec<Series>, PacsConnectorError> {
        Err(PacsConnectorError::UnsupportedOperation {
            pacs_id: "unknown".to_string(),
            operation: "search_series",
            reason: "dcm4chee2183 + dicomweb metadata provider does not support series search yet",
        })
    }

    async fn search_instances(&self, _criteria: &InstanceSearchCriteria) -> Result<Vec<Instance>, PacsConnectorError> {
        Err(PacsConnectorError::UnsupportedOperation {
            pacs_id: "unknown".to_string(),
            operation: "search_instances",
            reason: "dcm4chee2183 + dicomweb metadata provider does not support instance search yet",
        })
    }
}
