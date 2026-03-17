use serde::Serialize;

// --------------------------------------------------------------------- //
// -- OHIF Model
// --------------------------------------------------------------------- //

#[derive(Serialize, Debug)]
pub struct OhifStudies {
    pub studies: Box<Vec<OhifStudy>>
}

/// OHIF Study Model
#[derive(Serialize, Debug)]
pub struct  OhifStudy {
    #[serde(skip)]
    pub study_pk: i32,
    
    /// Mandatory DICOM attribute
    #[serde(rename="StudyInstanceUID")]
    pub study_iuid: String,

    #[serde(rename="StudyDate")]
    pub study_date: String,

    #[serde(rename="StudyTime")]
    pub study_time: String,

    #[serde(rename="StudyDescription")]
    pub study_description: Option<String>,

    #[serde(rename="PatientName")]
    pub patient_name: String,

    #[serde(rename="PatientID")]
    pub patient_id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="AccessionNumber")]
    pub accession_no: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="PatientAge")]
    pub patient_age: Option<String>,

    #[serde(rename="PatientSex")]
    pub patient_sex: Option<String>,

    #[serde(rename="NumInstances")]
    pub num_instances: i32,

    #[serde(rename="Modalities")]
    pub modalities: String,

    /// DICOM (0008,0080) InstitutionName
    ///
    /// Optional in our output: when absent/unknown, OHIF can still render.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="InstitutionName")]
    pub institution_name: Option<String>,

    pub series: Vec<OhifSerie>,
}
impl PartialEq for OhifStudy {
    fn eq(&self, other: &Self) -> bool {
        self.study_pk == other.study_pk //&& self.study_iuid == other.study_iuid
    }
}
impl PartialEq<i32> for OhifStudy {
    fn eq(&self, pk: &i32) -> bool {
        self.study_pk.eq(pk)
    }
}

/// OHIF Series Model
#[derive(Serialize, Debug)]
pub struct OhifSerie {
    #[serde(skip)]
    pub serie_pk: i32,

    /// Mandatory DICOM attribute
    #[serde(rename="SeriesInstanceUID")]
    pub series_iuid: String,

    #[serde(rename="SeriesNumber")]
    pub series_no: i32,

    /// Mandatory DICOM attribute
    #[serde(rename="Modality")]
    pub modality: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="SeriesDescription")]               
    pub series_description: Option<String>,  

    pub instances: Vec<OhifInstance>,
}
impl PartialEq for OhifSerie {
    fn eq(&self, other: &Self) -> bool {
        self.serie_pk == other.serie_pk //&& self.series_iuid == other.series_iuid 
    }
}
impl PartialEq<i32> for OhifSerie {
    fn eq(&self, pk: &i32) -> bool {
        self.serie_pk.eq(pk)
    }
}

#[derive(Serialize, Debug)]
pub struct OhifInstance {
    #[serde(skip)]
    pub instance_pk: i32,
    pub metadata: OhifInstanceMetadata,
    pub url: String,
}
impl PartialEq for OhifInstance {
    fn eq(&self, other: &Self) -> bool {
        self.instance_pk == other.instance_pk //&& self.sop_iuid == other.sop_iuid
    }
}
impl PartialEq<i32> for OhifInstance {
    fn eq(&self, pk: &i32) -> bool {
        self.instance_pk.eq(pk)
    }
}

/// OHIF Instance Metadata Model
#[derive(Serialize, Debug)]
pub struct OhifInstanceMetadata {

    /// Mandatory DICOM attribute
    #[serde(rename="SOPInstanceUID")]
    pub instance_sop_iuid: String,

    /// Optional but recommended DICOM attribute
    #[serde(rename="InstanceNumber")]
    pub instance_no: i32,

    /// Mandatory DICOM attribute
    #[serde(rename="SOPClassUID")]
    pub instance_sop_cuid: String,
    
    /// Mandatory DICOM attribute
    #[serde(rename="Modality")]
    pub series_modality: String,

    #[serde(rename="SeriesInstanceUID")]
    pub series_iuid: String,

    #[serde(rename="StudyInstanceUID")]
    pub study_iuid: String,

    #[serde(rename="SeriesDate")]
    pub series_date: String,

    // Extra attributes fetched from database or file

    /// Mandatory DICOM attribute
    #[serde(rename="PixelRepresentation")]      // DicomTag (0028,0103)	US
    pub pixel_representation: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="SamplesPerPixel")]          // DicomTag (0028,0002)	US OPTIONAL
    pub samples_per_pixel: Option<u16>,         

    /// Mandatory DICOM attribute
    #[serde(rename="PixelSpacing")]             // DicomTag (0028,0030)	DS OPTIONAL
    pub pixel_spacing: Option<Vec<f64>>,        // Maybe Vec<f64> ????

    /// Mandatory DICOM attribute
    #[serde(rename="Columns")]                   // DicomTag (0028,0011) US REQUIRED
    pub columns: Option<u16>,

    /// Mandatory DICOM attribute
    #[serde(rename="Rows")]                      // DicomTag (0028,0010) US REQUIRED
    pub rows: Option<u16>,
    
    /// Mandatory DICOM attribute
    #[serde(rename="PhotometricInterpretation")] // DicomTag (0028,0004) CS REQUIRED
    pub photometric_interpretation: Option<String>,
 
    /// Mandatory DICOM attribute
    #[serde(rename="BitsAllocated")]             // DicomTag (0028,0100) US REQUIRED
    pub bits_allocated: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="BitsStored")]                // DicomTag (0028,0101) US OPTIONAL
    pub bits_stored: Option<u16>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="HighBit")]                   // DicomTag (0028,0102) US OPTIONAL
    pub high_bit: Option<u16>,
   
    // ------------------------------------------------------------------------------ //
    // Required for MPR (Multi-Planar Reformatting) rendering and tools

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="ImageOrientationPatient")]   // DicomTag (0020,0037) DS OPTIONAL
    pub image_orientation_patient: Option<Vec<f64>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="ImagePositionPatient")]      // DicomTag (0020,0032) DS OPTIONAL
    pub image_position_patient: Option<Vec<f64>>,

    // ------------------------------------------------------------------------------ //

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="FrameOfReferenceUID")]          // DicomTag (0020,0052) UI OPTIONAL
    pub frame_of_reference_uid: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="ImageType")]                    // DicomTag (0008,0008) CS OPTIONAL
    pub image_type: Option<Vec<String>>,
   
    // ------------------------------------------------------------------------------ //
    // Optional DICOM attribute Required for proper rendering of images

    /// Optional DICOM attribute Required for proper rendering of images
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="WindowCenter")]                 // DicomTag (0028,1050) DS OPTIONAL
    pub window_center: Option<f64>,                 // Maybe Vec<f64> ????

    /// Optional DICOM attribute Required for proper rendering of images
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="WindowWidth")]                  // DicomTag (0028,1051) DS OPTIONAL
    pub window_width: Option<f64>,                  // Maybe Vec<f64> ????

    /// Optional DICOM attribute Required for proper rendering of images
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="RescaleIntercept")]              // DicomTag (0028,1052) DS OPTIONAL
    pub rescale_intercept: Option<f64>,              // Maybe Vec<f64> ????
    
    /// Optional DICOM attribute Required for proper rendering of images
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="RescaleSlope")]                  // DicomTag (0028,1053) DS OPTIONAL
    pub rescale_slope: Option<f64>,                  // Maybe Vec<f64> ????
    
    // ------------------------------------------------------------------------------ //

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="PlanarConfiguration")]          // DicomTag (0028,0006) US
    pub planar_configuration: Option<u16>,
    
    /// Optional DICOM attribute, Required for multi-frame images (US, CT, MR)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="NumberOfFrames")]               // DicomTag (0028,0008) IS
    pub number_of_frames: Option<u32>,

    // ------------------------------------------------------------------------------ //
    // Optional DICOM attribute, Required for US Modality

    // Optional DICOM attribute, Required for ultrasound images
    // #[serde(skip_serializing_if = "Option::is_none")]
    // #[serde(rename="SequenceOfUltrasoundRegions")]       // DicomTag (0018,6011) SQ
    // pub seq_of_ultrasound_regions: Option<u16>,

    // Optional DICOM attribute, Required for ultrasound images
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename="FrameTime")]                     // DicomTag (0018,1063) DS
    pub frame_time: Option<f64>,    

    // ------------------------------------------------------------------------------ //
    // Optional DICOM attribute, Required for PT Modality

    // #[serde(skip_serializing_if = "Option::is_none")]
    // #[serde(rename="RadiopharmaceuticalInformationSequence")] // DicomTag (0054,0016) SQ
    // pub radiopharmaceutical_info_seq: Option<u16>,
    


}