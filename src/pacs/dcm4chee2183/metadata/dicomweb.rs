use async_trait::async_trait;
use reqwest::{Client, Url};
use serde_json::Value;

use crate::{
    features::study_token::entities::Study,
    pacs::{
        Instance, InstanceSearchCriteria, MetadataProvider, Series, SeriesSearchCriteria, StudySearchCriteria
    },
};

#[derive(Clone)]
pub struct Dcm4chee2183DicomWebMetadataProvider {
    http_client: Client,
    base_url: String,
}

impl Dcm4chee2183DicomWebMetadataProvider {
    pub fn new(http_client: Client, base_url: String) -> Self {
        Self {
            http_client,
            base_url,
        }
    }

    fn build_qido_studies_url(&self, criteria: &StudySearchCriteria) -> anyhow::Result<Url> {
        let mut url = Url::parse(&self.base_url)?;

        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow::anyhow!("invalid dicomweb base URL"))?;
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


fn dicom_json_first_string(row: &Value, tag: &str) -> Option<String> {
    row.get(tag)
        .and_then(|node| node.get("Value"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.as_str().map(ToString::to_string))
}

fn dicom_json_patient_name(row: &Value) -> Option<String> {
    row.get("00100010")
        .and_then(|node| node.get("Value"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.get("Alphabetic"))
        .and_then(|alphabetic| alphabetic.as_str().map(ToString::to_string))
        .or_else(|| dicom_json_first_string(row, "00100010"))
}


#[async_trait]
impl MetadataProvider for Dcm4chee2183DicomWebMetadataProvider {

    async fn search_studies(&self, criteria: &StudySearchCriteria) -> anyhow::Result<Vec<Study>> {
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
        anyhow::bail!("dcm4chee2183 + dicomweb metadata provider does not support study search yet")

    }

    async fn search_series(&self, _criteria: &SeriesSearchCriteria) -> anyhow::Result<Vec<Series>> {
        anyhow::bail!("dcm4chee2183 + dicomweb metadata provider does not support series search yet")
    }

    async fn search_instances(&self, _criteria: &InstanceSearchCriteria) -> anyhow::Result<Vec<Instance>> {
        anyhow::bail!("dcm4chee2183 + dicomweb metadata provider does not support instance search yet")
    }
}
