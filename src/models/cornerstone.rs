use sqlx::MySqlPool;
use anyhow::Result;
use serde_json::{Value,json};

use crate::{api::study_token::params::StudyTokenParams, settings::Settings, database};


// --------------------------------------------------------------------- //
// -- Cornerstone Model
// --------------------------------------------------------------------- //

#[derive(Debug)]
pub struct Studies {
    pub inner: Box<Vec<Patient>>
}

#[derive(Debug)]
pub struct Patient {
    pub pat_pk: i32,
    pub pat_id: String,
    pub pat_name: String,
    pub pat_birthdate: Option<String>,
    pub pat_sex: Option<String>,
    pub studies: Vec<Study>,
}
impl PartialEq for Patient {
    fn eq(&self, other: &Self) -> bool {
        self.pat_pk == other.pat_pk //&& self.pat_id == other.pat_id 
    }
}
impl PartialEq<i32> for Patient {
    fn eq(&self, pk: &i32) -> bool {
        self.pat_pk.eq(pk)
    }
}

#[derive(Debug)]
pub struct  Study {
    pub study_pk: i32,
    pub study_iuid: String,
    pub study_date: String,
    pub study_time: String,
    pub accession_no: Option<String>,
    pub study_desc: Option<String>,
    pub mods_in_study: String,
    pub institution: Option<String>,
    pub physicians_reading: Option<String>,
    pub ref_physician: Option<String>,
    pub study_id: Option<String>,
    pub series: Vec<Serie>,
}
impl PartialEq for Study {
    fn eq(&self, other: &Self) -> bool {
        self.study_pk == other.study_pk //&& self.study_iuid == other.study_iuid
    }
}
impl PartialEq<i32> for Study {
    fn eq(&self, pk: &i32) -> bool {
        self.study_pk.eq(pk)
    }
}


#[derive(Debug)]
pub struct Serie {
    pub serie_pk: i32,
    pub series_iuid: String,
    pub series_no: String,
    pub station_name: Option<String>,
    pub series_desc: Option<String>,
    pub modality: String,
    pub perf_physician: Option<String>,
    pub num_instances: i32,
    pub instances: Vec<Instance>,
}
impl PartialEq for Serie {
    fn eq(&self, other: &Self) -> bool {
        self.serie_pk == other.serie_pk //&& self.series_iuid == other.series_iuid 
    }
}
impl PartialEq<i32> for Serie {
    fn eq(&self, pk: &i32) -> bool {
        self.serie_pk.eq(pk)
    }
}


#[derive(Debug)]
pub struct Instance {
    pub instance_pk: i32,
    pub sop_iuid: String,
    pub inst_no: String,
    pub sop_cuid: String,
    pub num_frames: i32,
}
impl PartialEq for Instance {
    fn eq(&self, other: &Self) -> bool {
        self.instance_pk == other.instance_pk //&& self.sop_iuid == other.sop_iuid
    }
}
impl PartialEq<i32> for Instance {
    fn eq(&self, pk: &i32) -> bool {
        self.instance_pk.eq(pk)
    }
}


impl Patient {
    fn cornerstone_manifest(&self, params: &StudyTokenParams, settings: &Settings, server_base_url: &String) -> Value {
        json!({
            "PatientID": self.pat_id,
            "PatientName":self.pat_name,
            "PatientBirthDate": self.pat_birthdate,
            "PatientSex":self.pat_sex,
            "key":self.pat_pk,
            "studyList": self.studies.iter().map(|x| x.cornerstone_manifest(params, settings, self, server_base_url)).collect::<Vec<Value>>()
        })
    }

}

impl Study {
    fn cornerstone_manifest(&self, params: &StudyTokenParams, settings: &Settings, pat: &Patient, server_base_url: &String) -> Value {
        json!({
            "NameOfPhysiciansReadingStudy": self.physicians_reading,
            "StudyInstanceUID": self.study_iuid,
            "studyDate":self.study_date.replace('-',""),      // Change format to AAAAMMDD
            "AccessionNumber":self.accession_no,
            "studyDescription": self.study_desc,
            "key": self.study_pk,                                 
            "modality": self.mods_in_study,
            "patientName": pat.pat_name,
            "StudyTime":self.study_time.replace(':',""),      // Change format to HHMMSS
            "institution":self.institution,
            "ReferringPhysicianName":self.ref_physician,
            "patientId": pat.pat_id,
            "StudyID": self.study_id,
            "seriesList": self.series.iter().map(|x| x.cornerstone_manifest(params, settings, self, server_base_url)).collect::<Vec<Value>>()
        })
    }

}


impl Serie {
    fn cornerstone_manifest(&self, params: &StudyTokenParams, settings: &Settings, study: &Study, server_base_url: &String) -> Value {
        json!({
            "seriesNumber": self.series_no,
            "SeriesInstanceUID": self.series_iuid,
            "SOPClassUID": self.instances.get(0).map(|ins| &ins.sop_cuid), //take sop_cuid of the first instance or null if not exists
            "WadoTransferSyntaxUID":"*",
            "StationName": self.station_name,
            "seriesDescription": self.series_desc,
            "numImages": self.num_instances,
            "Modality":self.modality,
            "PerformingPhysician": self.perf_physician,
            "Institution": study.institution,
            "key": self.serie_pk,
            "instanceList": self.instances.iter().map(|x| x.cornerstone_manifest(params, settings, study, self, server_base_url)).collect::<Vec<Value>>()
        })
    }
}

impl Instance {
    fn cornerstone_manifest(&self, params: &StudyTokenParams, settings: &Settings, study: &Study, serie: &Serie, server_base_url: &String) -> Value {
        let mut image_id = format!("wadouri:{}?requestType=WADO&studyUID={}&seriesUID={}&objectUID={}&contentType=application/dicom&transferSyntax={}",
            params.proxy_uri.as_ref()
                .unwrap_or(&settings.dicomarchive.manifest_base_url.as_ref()
                .unwrap_or(&server_base_url)),
            study.study_iuid, 
            serie.series_iuid, 
            self.sop_iuid,
            settings.dicomarchive.transfer_syntax,
        );

        if let Some(value) = &params.session {
            image_id.push_str(format!("&session={value}").as_ref());
        }

        if let Some(value) = &settings.dicomarchive.custodianoid {
            image_id.push_str(format!("&custodianOID={value}").as_ref());
        }

        if let Some(value) = &settings.dicomarchive.pacsoid {
            image_id.push_str(format!("&arcId={value}").as_ref());
        }

        if let Some(value) = &params.token {
            image_id.push_str(format!("&token={value}").as_ref());
        }
        
        json!({
            "imageId": image_id,
            "InstanceNumber": self.inst_no,
            "key": self.instance_pk,
            "numFrames": self.num_frames,
            "SOPClassUID": self.sop_cuid,
            "SOPInstanceUID": self.sop_iuid
        })
    }
}


// --------------------------------------------------------------------- //
// -- Cornerstone main function
// --------------------------------------------------------------------- //

pub async fn build_manifest(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings, server_base_url: String) -> Result<Value> {

    let studies = database::get_cornerstone_studies(pool, params, settings).await?;

    // TODO - Use diferent thread web::block(...) to build the json
    Ok(json!([{
        "arcId": settings.dicomarchive.pacsoid,
        "baseUrl": params.proxy_uri.as_ref().unwrap_or(&settings.dicomarchive.manifest_base_url.as_ref().unwrap_or(&server_base_url)),
        "patientList": studies.inner.into_iter().map(|x| x.cornerstone_manifest(params, settings, &server_base_url)).collect::<Vec<Value>>()
    }]))
}
