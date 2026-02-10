use std::io::Cursor;

use quick_xml::{Writer, events::{Event, BytesDecl, BytesStart}};
use sqlx::MySqlPool;
use anyhow::Result;

use crate::{api::study_token::params::StudyTokenParams, 
            settings::Settings, database
};

// --------------------------------------------------------------------- //
// -- Weasis Data Model
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
    pub series_desc: Option<String>,
    pub modality: String,
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


// --------------------------------------------------------------------- //
// -- Weasis main function
// --------------------------------------------------------------------- //

pub async fn build_manifest(
    pool: &MySqlPool, 
    params: &StudyTokenParams, 
    settings: &Settings, 
    server_base_url: String) 
    -> Result<Cursor<Vec<u8>>> 
{

    let studies = database::get_weasis_studies(pool, params, settings).await?;

    // TODO - Use diferent thread web::block(...) to build the json
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // -- ROOT LEVEL ------------------------------------------------------------------------- //
    let event = Event::Decl(BytesDecl::from_start(BytesStart::from_content("xml version=\"1.0\" encoding=\"UTF-8\"", 0)));
    writer.write_event(event)?;

    let base_url = params.proxy_uri
        .as_ref()
        .unwrap_or(&settings.dicomarchive.manifest_base_url
            .as_ref()
            .unwrap_or(&server_base_url)
        ).as_str();
    
    let arc_id = match &settings.dicomarchive.pacsoid {
        Some(value) => value,
        None => "UNKNOWN"
    };

    writer
        .create_element("manifest")
        .with_attribute(("xmlns", "http://www.weasis.org/xsd/2.5")) 
        .with_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"))
  
        // -- ARC QUERY LEVEL ------------------------------------------------------------------------- //
        .write_inner_content(|writer| {
            writer.create_element("arcQuery")
                .with_attribute(("arcId", arc_id))
                .with_attribute(("baseUrl", base_url))

                // -- PATIENT LEVEL ------------------------------------------------------------------------- //
                .write_inner_content(|writer| {
                    for patient in studies.inner.iter() {
                        writer.create_element("Patient")
                            .with_attributes(patient_attrs(patient).iter().map(|x| (x.0, x.1.as_str())))

                            // -- STUDY LEVEL ------------------------------------------------------------------------- //
                            .write_inner_content(|writer| {
                                for study in patient.studies.iter() {
                                    writer
                                        .create_element("Study")   
                                        .with_attributes(study_attrs(study).iter().map(|x| (x.0, x.1.as_str())))

                                        // -- SERIES LEVEL ------------------------------------------------------------------------- //
                                        .write_inner_content(|writer| {
                                            for serie in study.series.iter() {
                                                writer
                                                    .create_element("Series")
                                                    .with_attributes(serie_attrs(serie))

                                                    // -- INSTANCE LEVEL ------------------------------------------------------------------------- //
                                                    .write_inner_content(|writer| {
                                                        for instance in serie.instances.iter() {
                                                            writer
                                                                .create_element("Instance")
                                                                .with_attributes(instance_attrs(instance).iter().map(|x| (x.0, x.1.as_str())))
                                                                .write_empty()?;
                                                        }
                                                        Ok(())
                                                    })?; 
                                            }
                                            Ok(())
                                        })?;
                                }
                                Ok(())
                            })?;
                    }
                    Ok(())
                 })?;

            Ok(())
        })?;

    Ok(writer.into_inner())
}



// --------------------------------------------------------------------- //
// -- Weasis Helpers functions
// --------------------------------------------------------------------- //

fn patient_attrs(patient: &Patient)-> Vec<(&str, String)> {
    let mut attrs: Vec<(&str, String)> = Vec::new();

    attrs.push(("PatientID", patient.pat_id.to_string()));
    attrs.push(("PatientName", patient.pat_name.to_string()));
    if let Some(sex) = &patient.pat_sex {
        attrs.push(("PatientSex", sex.to_string()));
    } 
    if let Some(bdate) = &patient.pat_birthdate {
        attrs.push(("PatientBirthDate", bdate.replace('-',"")));
    }
    attrs
}


fn study_attrs(study: &Study)-> Vec<(&str, String)> {
    let mut attrs: Vec<(&str, String)> = Vec::new();

    attrs.push(("StudyInstanceUID", study.study_iuid.to_string()));
    if let Some(study_desc) = &study.study_desc {
        attrs.push(("StudyDescription", study_desc.to_string()));
    }
    attrs.push(("StudyDate", study.study_date.replace('-',"")));
    attrs.push(("StudyTime", study.study_time.replace(':',""))); 
    if let Some(accession_no) = &study.study_desc {
        attrs.push(("AccessionNumber", accession_no.to_string()));
    }
    if let Some(study_id) = &study.study_id {
        attrs.push(("StudyID", study_id.to_string()));
    }
    if let Some(ref_phy) = &study.ref_physician {
        attrs.push(("ReferringPhysicianName", ref_phy.to_string()));
    }
    attrs
}


fn serie_attrs(serie: &Serie)-> Vec<(&str, &str)> {
    let mut attrs: Vec<(&str, &str)> = Vec::new();

    attrs.push(("SeriesInstanceUID", &serie.series_iuid));
    attrs.push(("SeriesNumber", &serie.series_no));
    attrs.push(("Modality", &serie.modality));
    if let Some(series_desc) = &serie.series_desc {
        attrs.push(("SeriesDescription", series_desc));
    }
    attrs
}

fn instance_attrs(instance: &Instance)-> Vec<(&str, String)> {
    vec![
        ("SOPInstanceUID", instance.sop_iuid.to_string()),
        ("InstanceNumber", instance.inst_no.to_string()),
    ]
}

