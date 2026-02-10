use std::sync::{Arc, Mutex};

use actix_web::web;
use chrono::{NaiveDate, Utc};
use dicom_core::{Tag, DataElement};
use dicom_object::{InMemDicomObject, OpenFileOptions};
use dicom_dictionary_std::tags as DicomTag;
use sqlx::MySqlPool;
use anyhow::anyhow;

use crate::settings::Settings;

use super::DBVersion;




// --------------------------------------------------------------------- //
// -- DATABASE HELPER FUNCTIONS
// --------------------------------------------------------------------- //

/// TODO: Describre
pub async fn get_dicom_element<'a>(
    tag: Tag, 
    filepath: Arc<Mutex<String>>, 
    db_dicomattrs: &'a InMemDicomObject, 
    file_dicomattrs: &'a mut InMemDicomObject, 
    instance_pk: i32, 
    pool: &MySqlPool, 
    settings: &Settings)
    -> anyhow::Result<Option<&'a DataElement<InMemDicomObject, Vec<u8>>>> 
{
    match db_dicomattrs.element_opt(tag)? {
        Some(value) => Ok(Some(value)),
        None => {
            log::warn!("Could not get value of DicomTag `{}` from database.",tag);

            let mut c_filepath = filepath.lock().unwrap();

            //-- Fetch instance filepath if not already fetched
            if c_filepath.is_empty() {
                let row: (String, i32) = match &settings.dicomarchive.version {
                    DBVersion::dcm4chee2183 => 
                        sqlx::query_as("SELECT files.filepath, files.filesystem_fk FROM files WHERE instance_fk = ?")
                            .bind(instance_pk)
                            .fetch_one(pool).await?,

                    DBVersion::dcm4chee440 => sqlx::query_as("SELECT file_ref.filepath, file_ref.filesystem_fk FROM file_ref WHERE instance_fk = ?")
                            .bind(instance_pk)
                            .fetch_one(pool).await?,
                };
                
                c_filepath.push_str(format!("{}/{}",
                    settings.dicomarchive.get_fs_path_by_id(row.1)
                        .ok_or(anyhow!("Not found mapping for dcm4chee filesystem id: `{}`",row.1))?,
                    row.0,  
                ).as_str());
            } 

            //-- Fetch instance attributes from dicom file if not already fetched 
            if file_dicomattrs.tags().count() == 0 {
                log::debug!("Read dicom file `{}`",c_filepath);

                // -- Use a separate thread for blocking operation (read file)
                // ----------------------------------------------------------//
                drop(c_filepath); // Unlock a mutex guard
                let c_filepath = filepath.clone();
                
                *file_dicomattrs = web::block(move||{
                    let path = &*c_filepath
                        .lock()
                        .map_err(|err| anyhow!("Failed to get r/w lock for a file: {err}"))?;
                    
                    OpenFileOptions::new()
                        .read_until(DicomTag::PIXEL_DATA)
                        .open_file(path)
                        .map_err(|err| anyhow!(err))
                }).await??
                .into_inner()          
            }

            //-- Fetch instance dicom attributes from file if not already fetched
            match file_dicomattrs.element_opt(tag)? {
                Some(value) => Ok(Some(value)),
                None => { 
                    log::warn!("Failed to fetch DicomTag {tag} from database and from dicom file");
                   Ok(None)   
                }
            }
        }
    }            
}

/// Calculate patient age.
/// 
/// Spected value `AAAAMMDD`. It remove all non-digit characters from the `birthdate` value to 
/// parse and calculate the age
pub fn calculate_age(birthdate: String) -> anyhow::Result<i64> {

    // Parse the date of birth
    // Remove all non-digit from the value
    let birthdate:String = birthdate.chars().filter(|c| c.is_digit(10)).collect();
    let birthdate = NaiveDate::parse_from_str(&birthdate, "%Y%m%d")?;

    // Get the current date
    let current_date = Utc::now().naive_utc();

    // Calculate the age
    let age = current_date.date() - birthdate;
    let age = age.num_days()/365;
    Ok(age)
}



/// Pretty print sql queries
pub fn _prettysql(query: &str) -> String {
    format!("\n{}",
        sqlformat::format(
            query,
            &sqlformat::QueryParams::None, 
            sqlformat::FormatOptions::default()
        )
    )
}