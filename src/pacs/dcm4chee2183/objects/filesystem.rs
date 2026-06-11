use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;

use crate::pacs::{
	DicomObject,
	InstanceLocator,
	ObjectAccessContext,
	ObjectProvider,
	PacsConnectorError,
};


#[allow(dead_code)]
#[derive(Clone)]
pub struct Dcm4chee2183FilesystemObjectProvider {
	pacs_id: String,
	roots_by_filesystem: HashMap<i32, PathBuf>,
}

#[allow(dead_code)]
impl Dcm4chee2183FilesystemObjectProvider {
	pub fn new(pacs_id: String, roots_by_filesystem: HashMap<i32, PathBuf>) -> Self {
		Self { 
			pacs_id,
			roots_by_filesystem 
		}
	}

	fn resolve_instance_path(&self, locator: &InstanceLocator) -> Result<PathBuf, PacsConnectorError> {
		let filesystem_id = locator
			.filesystem_id
			.ok_or(PacsConnectorError::MissingField {
				pacs_id: self.pacs_id.clone(),
				field: "filesystem_id",
				operation: "retrieve_instance",
			})?;

		let root = self.roots_by_filesystem.get(&filesystem_id).ok_or(
			PacsConnectorError::FilesystemMappingMissing { 
				pacs_id: self.pacs_id.clone(),
				filesystem_id }
		)?;

		let relative_file_path = locator
			.relative_file_path
			.as_deref()
			.ok_or(PacsConnectorError::MissingField {
				pacs_id: self.pacs_id.clone(),
				field: "relative_file_path",
				operation: "retrieve_instance",
			})?;

		let relative_file_path = relative_file_path.trim_start_matches('/');

		Ok(root.join(relative_file_path))
	}
}


#[async_trait]
impl ObjectProvider for Dcm4chee2183FilesystemObjectProvider {
	
	async fn retrieve_instance(&self, locator: &InstanceLocator) -> Result<DicomObject, PacsConnectorError> {
		let full_path = self.resolve_instance_path(locator)?;

		let bytes = tokio::fs::read(&full_path).await.map_err(|source| PacsConnectorError::Io {
			pacs_id: self.pacs_id.clone(),
			source,
		})?;

		Ok(DicomObject {
			bytes,
			content_type: "application/dicom".to_string(),
		})
	}
	
	fn build_access_link(
		&self,
		locator: &InstanceLocator,
		_context: &ObjectAccessContext,
	) -> Result<String, PacsConnectorError> {
		let full_path = self.resolve_instance_path(locator)?;
		Ok(format!("file://{}", full_path.display()))
	}
}

