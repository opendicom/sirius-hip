use anyhow::Context;
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
};


#[derive(Clone)]
pub struct Dcm4chee2183DicomWebObjectProvider {
	http_client: Client,
	base_url: String,
}

impl Dcm4chee2183DicomWebObjectProvider {
	pub fn new(http_client: Client, base_url: String) -> Self {
		Self {
			http_client,
			base_url,
		}
	}

	fn build_wado_rs_url(
		&self,
		locator: &InstanceLocator,
		context: &ObjectAccessContext,
	) -> anyhow::Result<String> {
		let mut url = Url::parse(&self.base_url)
			.with_context(|| format!("invalid DICOMweb URL configured: {}", self.base_url))?;

		{
			let mut segments = url
				.path_segments_mut()
				.map_err(|_| anyhow::anyhow!("invalid dicomweb base URL"))?;

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
	
	async fn retrieve_instance(&self, locator: &InstanceLocator) -> anyhow::Result<DicomObject> {
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
			.await?
			.error_for_status()?;

		let content_type = response
			.headers()
			.get(CONTENT_TYPE)
			.and_then(|value| value.to_str().ok())
			.unwrap_or("application/dicom")
			.to_string();

		let bytes = response.bytes().await?.to_vec();

		Ok(DicomObject {
			bytes,
			content_type,
		})
	}
	
	fn build_access_link(
		&self,
		locator: &InstanceLocator,
		context: &ObjectAccessContext,
	) -> anyhow::Result<String> {
		self.build_wado_rs_url(locator, context)
	}
}
