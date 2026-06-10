use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, anyhow};
use async_trait::async_trait;

use crate::pacs::{
	DicomObject,
	InstanceLocator,
	ObjectAccessContext,
	ObjectProvider,
};


#[allow(dead_code)]
#[derive(Clone)]
pub struct Dcm4chee2183FilesystemObjectProvider {
	roots_by_filesystem: HashMap<i32, PathBuf>,
}

#[allow(dead_code)]
impl Dcm4chee2183FilesystemObjectProvider {
	pub fn new(roots_by_filesystem: HashMap<i32, PathBuf>) -> Self {
		Self { roots_by_filesystem }
	}

	fn resolve_instance_path(&self, locator: &InstanceLocator) -> anyhow::Result<PathBuf> {
		let filesystem_id = locator
			.filesystem_id
			.ok_or_else(|| anyhow!("filesystem_id is required for filesystem object retrieval"))?;

		let root = self.roots_by_filesystem.get(&filesystem_id).ok_or_else(|| {
			anyhow!(
				"filesystem mapping is not configured for filesystem id {filesystem_id}"
			)
		})?;

		let relative_file_path = locator
			.relative_file_path
			.as_deref()
			.ok_or_else(|| anyhow!("relative_file_path is required for filesystem retrieval"))?;

		let relative_file_path = relative_file_path.trim_start_matches('/');

		Ok(root.join(relative_file_path))
	}
}


#[async_trait]
impl ObjectProvider for Dcm4chee2183FilesystemObjectProvider {
	
	async fn retrieve_instance(&self, locator: &InstanceLocator) -> anyhow::Result<DicomObject> {
		let full_path = self.resolve_instance_path(locator)?;

		let bytes = tokio::fs::read(&full_path)
			.await
			.with_context(|| format!("failed to read DICOM file at {}", full_path.display()))?;

		Ok(DicomObject {
			bytes,
			content_type: "application/dicom".to_string(),
		})
	}
	
	fn build_access_link(
		&self,
		locator: &InstanceLocator,
		_context: &ObjectAccessContext,
	) -> anyhow::Result<String> {
		let full_path = self.resolve_instance_path(locator)?;
		Ok(format!("file://{}", full_path.display()))
	}
}

