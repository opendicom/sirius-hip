use anyhow::Context;
use async_trait::async_trait;
use reqwest::{Client, Url, header::CONTENT_TYPE};

use crate::pacs::{
	DicomObject,
	InstanceLocator,
	ObjectAccessContext,
	ObjectProvider,
};


pub struct Dcm4chee2183DicomWebObjectProvider {
	http_client: Client,
	wadouri: String,
}

impl Dcm4chee2183DicomWebObjectProvider {
	pub fn new(http_client: Client, wadouri: String) -> Self {
		Self {
			http_client,
			wadouri,
		}
	}

	fn build_wado_url(
		&self,
		locator: &InstanceLocator,
		context: &ObjectAccessContext,
	) -> anyhow::Result<String> {
		let mut url = Url::parse(&self.wadouri)
			.with_context(|| format!("invalid WADO URI configured: {}", self.wadouri))?;

		{
			let mut query = url.query_pairs_mut();

			query.append_pair("requestType", "WADO");
			query.append_pair("studyUID", &locator.study_uid);
			query.append_pair("seriesUID", &locator.series_uid);
			query.append_pair("objectUID", &locator.sop_instance_uid);

			if let Some(content_type) = context.content_type.as_deref() {
				query.append_pair("contentType", content_type);
			}

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
		let url = self.build_access_link(locator, &ObjectAccessContext::default())?;

		let response = self
			.http_client
			.get(url)
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
		self.build_wado_url(locator, context)
	}
}
