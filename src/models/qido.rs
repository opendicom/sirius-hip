use std::collections::HashSet;

use dicom_json::DicomJson;
use dicom_object::InMemDicomObject;
use serde::Serialize;
use sqlx::MySqlPool;

use crate::{api::qido::QidoStudiesParams, settings::Settings, database};



// --------------------------------------------------------------------- //
// -- Qido Model
// --------------------------------------------------------------------- //

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



/*
// Qido implementation for serde_json::Value
// 
#[derive(Serialize, Debug)]
pub struct Qido(Vec<serde_json::Value>);
impl Qido {
    pub fn new() -> Qido{
        Qido(vec![])
    }

    pub fn add_dicom_json(&mut self, dicom_json: serde_json::Value) {
        self.0.push(dicom_json);
    }
}
*/

// --------------------------------------------------------------------- //
// -- Qido main function
// --------------------------------------------------------------------- //

pub async fn get_studies(
    params: QidoStudiesParams, 
    validated_include_fields: HashSet<&'static str>, 
    pool: &MySqlPool, 
    settings: &Settings
) -> anyhow::Result<Qido>  {
    Ok(database::get_qido_studies(pool, validated_include_fields, &params, settings).await?)
 }


/*use serde::{Serialize, Serializer, ser::SerializeMap};
use sqlx::MySqlPool;

use crate::{settings::Settings, api::qido::QidoStudiesParams, database};


#[derive(Debug)]
pub struct DicomAttribute<'a, T: Serialize> {
    tag: &'a str,
    value: T,
}

#[derive(Serialize, Debug)]
pub struct Value<'a, T: Serialize> {
    vr: &'a str,
    value: Vec<T>,
}

impl<'a, T: Serialize> Serialize for DicomAttribute<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_map(Some(2))?;
        state.serialize_entry(&self.tag, &self.value)?;
        state.end()
    }
}


#[derive(Serialize)]
pub struct Qido(Vec<Box<dyn erased_serde::Serialize>>);

impl Qido {
    pub fn new() -> Self {
        Qido(vec![])
    }

    pub async fn get_studies(params: QidoStudiesParams, pool: &MySqlPool, settings: &Settings) -> anyhow::Result<Qido>  {
       Ok(database::get_qido_studies(pool, &params, settings).await?)
    }
}

pub fn test_qido(){

    let attr1 = DicomAttribute {
        tag: "00080005",
        value: Value {  
            vr: "CS",
            value: vec!["ISO_IR 100"],
        },
    };

    let attr2 = DicomAttribute {
        tag: "00080051",
        value: Value {  
            vr: "SQ",
            value: vec![
                DicomAttribute { 
                    tag: "UT", 
                    value: "2.16.858.2.10001442.72769.1" 
                }   
            ],
        },
    };

    let response = Qido(vec![Box::new(attr1), Box::new(attr2)]);

    println!("{}",serde_json::to_string(&response).unwrap());


}



// --------------------------------------------------------------------- //
// -- TEST Module
// --------------------------------------------------------------------- //

#[cfg(test)]
mod tests {
    use crate::models::qido::{DicomAttribute, Value, Qido};

    #[test]
    fn serialize_simple_value() {

        let attr = DicomAttribute {
            tag: "00080005",
            value: Value {  
                vr: "CS",
                value: vec!["ISO_IR 100"],
            },
        };
        let qido = Qido(vec![Box::new(attr)]);
        let json = serde_json::to_string(&qido).unwrap();
        assert_eq!(json, r#"[{"00080005":{"vr":"CS","value":["ISO_IR 100"]}}]"#)
    }

    #[test]
    fn serialize_secuence() {
        
        let attr = DicomAttribute {
            tag: "00080051",
            value: Value {  
                vr: "SQ",
                value: vec![
                    DicomAttribute { 
                        tag: "UT", 
                        value: "2.16.858.2.10001442.72769.1" 
                    }   
                ],
            },
        };
        let qido = Qido(vec![Box::new(attr)]);
        let json = serde_json::to_string(&qido).unwrap();
        assert_eq!(json, r#"[{"00080051":{"vr":"SQ","value":[{"UT":"2.16.858.2.10001442.72769.1"}]}}]"#)
    }
}

*/
