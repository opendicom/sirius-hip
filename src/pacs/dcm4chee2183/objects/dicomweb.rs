use async_trait::async_trait;
use reqwest::{
	Client,
	Url,
	header::{ACCEPT, CONTENT_TYPE},
};

use crate::pacs::{
	DicomObject,
	InstanceLocator,
	ObjectAccessContext,
	ObjectProvider,
	PacsConnectorError,
};


#[derive(Clone)]
pub struct Dcm4chee2183DicomWebObjectProvider {
	pacs_id: String,
	http_client: Client,
	base_url: String,
}

impl Dcm4chee2183DicomWebObjectProvider {
	pub fn new(pacs_id: String, http_client: Client, base_url: String) -> Self {
		Self {
			pacs_id,
			http_client,
			base_url,
		}
	}

	fn build_wado_rs_url(
		&self,
		locator: &InstanceLocator,
		context: &ObjectAccessContext,
	) -> Result<String, PacsConnectorError> {
		let mut url = Url::parse(&self.base_url).map_err(|_| PacsConnectorError::InvalidDicomwebBaseUrl {
			pacs_id: self.pacs_id.clone(),
			base_url: self.base_url.clone(),
		})?;

		{
			let mut segments = url
				.path_segments_mut()
				.map_err(|_| PacsConnectorError::InvalidDicomwebBaseUrl {
					pacs_id: self.pacs_id.clone(),
					base_url: self.base_url.clone(),
				})?;

			segments
				.push("studies")
				.push(&locator.study_uid)
				.push("series")
				.push(&locator.series_uid)
				.push("instances")
				.push(&locator.sop_instance_uid);
		}

		{
			let mut query = url.query_pairs_mut();

			if let Some(transfer_syntax) = context.transfer_syntax.as_deref() {
				query.append_pair("transferSyntax", transfer_syntax);
			}
		}

		Ok(url.to_string())
	}
}


#[async_trait]
impl ObjectProvider for Dcm4chee2183DicomWebObjectProvider {
	
	async fn retrieve_instance(&self, locator: &InstanceLocator) -> Result<DicomObject, PacsConnectorError> {
		let context = ObjectAccessContext::default();
		let url = self.build_access_link(locator, &context)?;
		let accept = context
			.content_type
			.as_deref()
			.unwrap_or("application/dicom");

		let response = self
			.http_client
			.get(url)
			.header(ACCEPT, accept)
			.send()
			.await
			.map_err(|source| PacsConnectorError::Reqwest { 
				pacs_id: self.pacs_id.clone(),
				source 
			})?
			.error_for_status()
			.map_err(|source| PacsConnectorError::Reqwest { 
				pacs_id: self.pacs_id.clone(),
				source 
			})?;

		let content_type = response
			.headers()
			.get(CONTENT_TYPE)
			.and_then(|value| value.to_str().ok())
			.unwrap_or("application/dicom")
			.to_string();

		let bytes = response.bytes()
		.await
		.map_err(|source| PacsConnectorError::Reqwest { 
			pacs_id: self.pacs_id.clone(),
			source 
		})?
		.to_vec();

		Ok(DicomObject {
			bytes,
			content_type,
		})
	}
	
	fn build_access_link(
		&self,
		locator: &InstanceLocator,
		context: &ObjectAccessContext,
	) -> Result<String, PacsConnectorError> {
		self.build_wado_rs_url(locator, context)
	}
}
