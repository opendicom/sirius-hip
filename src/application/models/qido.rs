use dicom_json::DicomJson;
use dicom_object::InMemDicomObject;
use serde::Serialize;


/// A wrapper around a vector of DicomJson objects, representing the result of a QIDO query.
#[derive(Serialize, Debug)]
pub struct Qido(Vec<DicomJson<InMemDicomObject>>);

impl Qido {
    pub fn new() -> Qido{
        Qido(vec![])
    }

    pub fn add_dicom_json(&mut self, dicom_json: DicomJson<InMemDicomObject>) {
        self.0.push(dicom_json);
    }
    
}
