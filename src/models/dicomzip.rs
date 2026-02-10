use sqlx::MySqlPool;
use anyhow::Result;

use crate::{api::study_token::params::StudyTokenParams, 
            settings::Settings, database
};

pub use self::streamzip::DicomStreamZip;


// --------------------------------------------------------------------- //
// -- DicomZIP Model
// --------------------------------------------------------------------- //

#[derive(Debug)]
pub struct Studies {
    pub inner: Box<Vec<Patient>>
}

#[derive(Debug)]
pub struct Patient {
    pub pat_pk: i32,    // Required
    pub pat_id: String,
    pub pat_name: String,
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
    pub study_pk: i32,  // Required
    pub study_iuid: String,
    pub study_desc: Option<String>,
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
    pub serie_pk: i32, // Required
    pub series_iuid: String,
    pub series_desc: Option<String>,
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
    pub retrieve_url: String, // Can be `file://path/to/file/` to access directly or `http://server/wado?...` to get by wado
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
// -- DicomZIP main function
// --------------------------------------------------------------------- //

pub async fn streamzip(pool: &MySqlPool, params: &StudyTokenParams, settings: &Settings) -> Result<DicomStreamZip> {

    let studies = database::get_dicomzip_studies(pool, params, settings).await?;

    let mut dicomzip = DicomStreamZip::new();
    let mut inst_counter: u32 = 0;

    for pat in *studies.inner {
        //dicomzip.append_dir(format!("/{}/",pat.pat_name));

        for study in pat.studies {
            //dicomzip.append_dir(format!("/{}/{}/",pat.pat_name,study.study_iuid));

            for serie in study.series {                
                //dicomzip.append_dir(format!("/{}/{}/{}",pat.pat_name,study.study_iuid,serie.series_iuid));

                for instance in serie.instances {
                    inst_counter += 1;
                    //let inst_name = format!("DICOM{:04}.dcm", inst_counter);
                    //dicomzip.append_file(format!("/{}/{}/{}/{}",pat.pat_name,study.study_iuid,serie.series_iuid,inst_name), instance.retrieve_url);
                    
                    //let inst_name = format!("DICOM_{:04}.dcm", inst_counter);
                    let inst_name = format!("{}.dcm", inst_counter);
                    dicomzip.add_entry(&inst_name, &instance.retrieve_url);
                }
            }
        }
    }

    Ok(dicomzip)
}



// --------------------------------------------------------------------- //
// -- StreamZIP Sub-module
// --------------------------------------------------------------------- //

/// References to build this module:
    ///     - https://en.wikipedia.org/wiki/ZIP_(file_format)
    ///     - https://libzip.org/specifications/extrafld.txt
    ///     - https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
mod streamzip {

    use actix_web::{web::Bytes, Error, error::ErrorInternalServerError};
    use async_stream::stream;
    use chrono::prelude::*;
    use futures::Stream;
    use crc32fast::Hasher;
    use std::io::Write;
    use tokio::io::AsyncReadExt;

    const CREATED_BY: &str = "Created by Opendicom - Sirius HIP (www.opendicom.com)";

    pub struct DicomStreamZip {  
        central_dir: Vec<u8>,
        entries_count: u16,
        curr_offset: u32,
        curr_lfh: Vec<u8>,
        build_entries: Vec<Entry>
    }

    #[derive(Debug)]
    pub struct Entry{
        name: String,
        url: String,
        crc: u32,
        size: u32,
        datetime:  DateTime<Local>,
    }
    impl Entry{

        /// Crate a new Zip Entry
        /// - `name` Name to use in the zip file
        /// - `url`  Used to retrive data. Can be:
        ///     - `file://...`
        ///     - `http/s://...`
        pub fn new(name: String, url: String) -> Self {
            Self { 
                name, 
                url, 
                crc: 0,
                size: 0,
                datetime: DateTime::<Local>::from(std::time::SystemTime::now()),
            }
        }

        /// Get name lenght as `bytes`
        pub fn name_lenght(&self) -> [u8;2] {
            (self.name.len() as u16).to_le_bytes()
        }

        /// Ge name to use in the zip file as `bytes`
        pub fn name(&self) -> &[u8] {
            &self.name.as_bytes()
        }

        /// Return a MS-DOS Date format as `bytes`
        ///
        /// https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-dosdatetimetovarianttime
        pub fn msdos_date(&self) -> [u8;2] {
            let date = (self.datetime.day() as u16) | ((self.datetime.month() as u16) << 5) | ((self.datetime.year() as u16 - 1980) << 9);
            date.to_le_bytes()
        }

        /// Return a MS-DOS Time format as `bytes`
        ///
        /// https://learn.microsoft.com/en-us/windows/win32/api/oleauto/nf-oleauto-dosdatetimetovarianttime
        pub fn msdos_time(&self) -> [u8;2] {
            let time = ((self.datetime.second() as u16) >> 1) | ((self.datetime.minute() as u16) << 5) | ((self.datetime.hour() as u16) << 11);
            time.to_le_bytes()
        }

        /// Return UNIX timestamp as `bytes`
        pub fn timestamp(&self) -> [u8;4] {
            (self.datetime.timestamp() as u32).to_le_bytes()
        }
    }

    impl DicomStreamZip {

        pub fn new() -> Self { 
            Self { 
                central_dir: Vec::new(),
                entries_count: 0,
                curr_offset: 0,
                curr_lfh: Vec::new(), //Current Local File Descriptor 
                build_entries: Vec::new(),
            }
        }

        pub fn add_entry(&mut self, name: &str, url: &str) {
            self.build_entries.push(Entry::new(name.to_string(), url.to_string()));
        }

        pub fn local_file_header(&mut self, entry: &Entry) -> Result<Bytes, Error> {

            // -------------------------------------------------------------------------------------------------- //
            // ---- Create zip Local File Header ---------------------------------------------------------------- //
                
            // Write Local File Header to current instance buffer
            self.curr_lfh.write_all(&[0x50, 0x4B, 0x03, 0x04])?;    // Offset  0:   Bytes: 4    Local file header signature = 0x04034b50 (PK♥♦ or "PK\3\4") 
            self.curr_lfh.write_all(&[0x0A, 0x00])?;                // Offset  4:   Bytes: 2    Version needed to extract (minimum)
            self.curr_lfh.write_all(&[0x08, 0x00])?;                // Offset  6:   Bytes: 2    General purpose bit flag (0x08 means CRC-32 and file sizes are not known when the header is written )
            self.curr_lfh.write_all(&[0x00, 0x00])?;                // Offset  8:   Bytes: 3    Compression method; e.g. none = 0, DEFLATE = 8 (or "\0x08\0x00")
            self.curr_lfh.write_all(&entry.msdos_time())?;          // Offset 10:   Bytes: 2    File last modification time
            self.curr_lfh.write_all(&entry.msdos_date())?;          // Offset 12:   Bytes: 2    File last modification date
            self.curr_lfh.write_all(&[0x00, 0x00, 0x00, 0x00])?;    // Offset 14:   Bytes: 4    CRC-32 of uncompressed data 
            self.curr_lfh.write_all(&[0x00, 0x00, 0x00, 0x00])?;    // Offset 18:   Bytes: 4    Compressed size (or 0xffffffff for ZIP64) 
            self.curr_lfh.write_all(&[0x00, 0x00, 0x00, 0x00])?;    // Offset 22:   Bytes: 4    Uncompressed size (or 0xffffffff for ZIP64)
            self.curr_lfh.write_all(&entry.name_lenght())?;         // Offset 26:   Bytes: 2    File name length (N)
            //self.curr_lfh.write_all(&[0x1C, 0x00])?;                // Offset 28:   Bytes: 2    Extra field length (M=28bytes)  
            self.curr_lfh.write_all(&[0x0D, 0x00])?;                // Offset 28:   Bytes: 2    Extra field length (M=28bytes)     
            self.curr_lfh.write_all(entry.name())?;                 // Offset 30:   Bytes: N    File name 
                                                                     
            // Extended Timestamp                                       // Extra Fields
            self.curr_lfh.write_all(&[0x55, 0x54])?;                    // 0x5455    Bytes: 2   Tag for this extra block type   ("UT")
            self.curr_lfh.write_all(&[0x09, 0x00])?;                    // TSize     Bytes: 2   Total data size for this block  (Size=9 bytes)
            self.curr_lfh.write_all(&[0x03])?;                          // Flags     Bytes: 1   Info bits
            self.curr_lfh.write_all(&entry.timestamp())?;               // ModTime   Bytes: 4   Time of last modification (UTC/GMT)
            self.curr_lfh.write_all(&entry.timestamp())?;               // AcTime    Bytes: 4   Time of last access (UTC/GMT)
        
            // // Info-ZIP New Unix                                        // Extra Fields
            // self.curr_lfh.write_all(&[0x75, 0x78])?;                    // 0x7875   Bytes: 2    tag for this extra block type ("ux")
            // self.curr_lfh.write_all(&[0x0B, 0x00])?;                    // TSize    Bytes: 2    total data size for this block
            // self.curr_lfh.write_all(&[0x01])?;                          // Version  Bytes: 1    version of this extra field, currently 1
            // self.curr_lfh.write_all(&[0x04])?;                          // UIDSize  Bytes: 1    Size of UID field
            // self.curr_lfh.write_all(&1000u32.to_le_bytes())?;           // UID      Bytes: 4   UID for this entry
            // self.curr_lfh.write_all(&[0x04])?;                          // GIDSize  Bytes: 1    Size of GID field
            // self.curr_lfh.write_all(&1000u32.to_le_bytes())?;           // GID      Bytes: 4    GID for this entry

            Ok(Bytes::copy_from_slice(&self.curr_lfh))

        }


        fn data_descriptor(&mut self, entry: &Entry) -> Result<Bytes, Error> {

            // ---- Create zip Data Descriptor ---------------------------------------------------------------- //
            // https://en.wikipedia.org/wiki/ZIP_(file_format)#Data_descriptor

            let mut buffer: Vec<u8> = Vec::new();
            buffer.write_all(&[0x50, 0x4B, 0x07, 0x08])?;   // Offset  0:   Bytes: 4  Optional data descriptor signature = 0x08074b50 
            buffer.write_all(&entry.crc.to_le_bytes())?;    // Offset  4:   Bytes: 4  CRC-32 of uncompressed data 
            buffer.write_all(&entry.size.to_le_bytes())?;   // Offset  8:   Bytes: 4  Compressed size 
            buffer.write_all(&entry.size.to_le_bytes())?;   // Offset 12:   Bytes: 4  Uncompressed size


            // -- Prepare central directory file header for this entry

            self.central_dir.write_all(&[0x50, 0x4B, 0x01, 0x02])?;         // Offset  0:    Bytes: 4  Central directory file header signature = 0x02014b50
            self.central_dir.write_all(&[0x1E, 0x03])?;                     // Offset  4:    Bytes: 2  Version made by 
            self.central_dir.write_all(&self.curr_lfh[4..14])?;             // Offset  6:    Bytes: -  Same as local file header Offset 4 to 16

            self.central_dir.write_all(&entry.crc.to_le_bytes())?;          // Offset 16:    Bytes: 4  CRC-32 of uncompressed data 
            self.central_dir.write_all(&entry.size.to_le_bytes())?;         // Offset 20:    Bytes: 4  Compressed size 
            self.central_dir.write_all(&entry.size.to_le_bytes())?;         // Offset 24:    Bytes: 4  Uncompressed size 
            
            self.central_dir.write_all(&entry.name_lenght())?;              // Offset 28:    Bytes: 2  File name length (N) 
            //self.central_dir.write_all(&[0x18, 0x00])?;                     // Offset 30:    Bytes: 2  Extra field length (M) 
            self.central_dir.write_all(&[0x09, 0x00])?;                     // Offset 30:    Bytes: 2  Extra field length (M) 
            self.central_dir.write_all(&[0x00, 0x00])?;                     // Offset 32:    Bytes: 2  File comment length (K)

            self.central_dir.write_all(&[0x00, 0x00])?;                     // Offset 34:    Bytes: 2  Disk number where file starts (or 0xffff for ZIP64) 
            self.central_dir.write_all(&[0x00, 0x00])?;                     // Offset 36:    Bytes: 2  Internal file attributes 
            self.central_dir.write_all(&[0x00, 0x00, 0xB4, 0x81])?;         // Offset 38:    Bytes: 4  External file attributes 
            self.central_dir.write_all(&self.curr_offset.to_le_bytes())?;   // Offset 42:    Bytes: 4  Relative offset of local file header.       
            
            self.central_dir.write_all(entry.name.as_bytes())?;             // Offset 46:    Bytes: N  File name

            // Extended Timestamp                                       // Extra Fields
            self.central_dir.write_all(&[0x55, 0x54])?;                    // 0x5455    Bytes: 2   Tag for this extra block type   ("UT")
            self.central_dir.write_all(&[0x05, 0x00])?;                    // TSize     Bytes: 2   Total data size for this block  (Size=9 bytes)
            self.central_dir.write_all(&[0x03])?;                          // Flags     Bytes: 1   Info bits
            self.central_dir.write_all(&entry.timestamp())?;               // ModTime   Bytes: 4   Time of last modification (UTC/GMT)
        
            // // Info-ZIP New Unix                                        // Extra Fields
            // self.central_dir.write_all(&[0x75, 0x78])?;                    // 0x7875   Bytes: 2    tag for this extra block type ("ux")
            // self.central_dir.write_all(&[0x0B, 0x00])?;                    // TSize    Bytes: 2    total data size for this block
            // self.central_dir.write_all(&[0x01])?;                          // Version  Bytes: 1    version of this extra field, currently 1
            // self.central_dir.write_all(&[0x04])?;                          // UIDSize  Bytes: 1    Size of UID field
            // self.central_dir.write_all(&1000u32.to_le_bytes())?;           // UID      Bytes: 4   UID for this entry
            // self.central_dir.write_all(&[0x04])?;                          // GIDSize  Bytes: 1    Size of GID field
            // self.central_dir.write_all(&1000u32.to_le_bytes())?;           // GID      Bytes: 4    GID for this entry
        
            self.curr_offset += self.curr_lfh.len() as u32 + entry.size + buffer.len() as u32;
            self.entries_count += 1;

            self.curr_lfh.clear();

            Ok(Bytes::copy_from_slice(&buffer))
        }


        fn end_of_central_directory(&mut self) ->  Result<Bytes, Error> {

         
            let central_dir_size = self.central_dir.len() as u32;

            // ------- End of central directory record (EOCD) 
            // https://en.wikipedia.org/wiki/ZIP_(file_format)#End_of_central_directory_record_(EOCD)
            
            let mut eocd: Vec<u8> = Vec::new();
            eocd.write_all(&[0x50, 0x4B, 0x05, 0x06])?;           // Offset  0:   Bytes: 4 End of central directory signature = 0x06054b50 
            eocd.write_all(&[0x00, 0x00])?;                       // Offset  4:   Bytes: 2 Number of this disk 
            eocd.write_all(&[0x00, 0x00])?;                       // Offset  6:   Bytes: 2 Disk where central directory starts
            eocd.write_all(&self.entries_count.to_le_bytes())?;   // Offset  8:   Bytes: 2 Number of central directory records on this disk
            eocd.write_all(&self.entries_count.to_le_bytes())?;   // Offset  10:  Bytes: 2 Total number of central directory records
            eocd.write_all(&central_dir_size.to_le_bytes())?;     // Offset  12:  Bytes: 4 Size of central directory (bytes)
            eocd.write_all(&self.curr_offset.to_le_bytes())?;     // Offset: 16   Bytes: 4 Offset of start of central directory, relative to start of archive
            
            eocd.write_all(&(CREATED_BY.len() as u16).to_le_bytes())?;  // Offset: 20   Bytes: 2 Comment length (n)
            eocd.write_all(&CREATED_BY.as_bytes())?;                    // Offset: 20   Bytes: 2 Comment length (n)
            
            self.central_dir.extend(eocd);
            
            Ok(Bytes::copy_from_slice(&self.central_dir))
        }

        
        pub fn build(mut self) -> impl Stream<Item = Result<Bytes, Error>> {

            self.build_entries.reverse();

            stream! {
                
                while let Some(mut entry) = self.build_entries.pop() {
                    log::debug!("Add file: {}",entry.url);

                    let mut hasher = Hasher::new();

                    // Stream local file header
                    yield self.local_file_header(&entry);

                    // --------------------------------------------------------------------------------------------------- //
                    // -- Get file from filesystem
                    if let Some(file) = entry.url.strip_prefix("file://") {

                        let mut handler = tokio::fs::File::open(file).await
                            .map_err(|e| { log::error!("Failed to get file from: `{}`\nCaused by: {}",entry.url,e); ErrorInternalServerError("") })?;
            
                        // Stream file in chunks and calculate crc checksum 
                        let mut buffer = [0;4096];
                        let mut bytesreaded;

                        loop { 
                            bytesreaded = handler.read(&mut buffer).await?;
                            if bytesreaded == 0 { // EOF
                                entry.crc = hasher.finalize();
                                break;

                            } else if bytesreaded <= buffer.len() {

                                hasher.update(&buffer[..bytesreaded]);
                                entry.size += bytesreaded as u32;

                                yield Ok(Bytes::copy_from_slice(&buffer[..bytesreaded]))
                            }
                        }
                    
                    // --------------------------------------------------------------------------------------------------- //
                    // -- Get file from wado
                    } else if entry.url.starts_with("http://") || entry.url.starts_with("https://") {
                       
                        // Stream file in chunks, calculate crc checksum and file_size
                        let mut handler = reqwest::get(&entry.url).await
                            .map_err(|e| { log::error!("Failed to get file from url: `{}`\nCaused by: {}",entry.url,e); ErrorInternalServerError("") })?;
                        
                        while let Some(chunk) = handler.chunk().await
                            .map_err(|e| { log::error!("Failed to get file from url: `{}`\nCaused by: {}",entry.url,e); ErrorInternalServerError("") })? {
                            
                            hasher.update(chunk.as_ref());
                            entry.size += chunk.len() as u32;
                            
                            yield Ok(chunk)
                        }
                        entry.crc = hasher.finalize();

                    } else {
                        log::error!("Unsupported protocol for retrieve file `{}`",entry.url);
                        yield Err(ErrorInternalServerError(""))
                    }

                    // Stream data descriptor
                    yield self.data_descriptor(&entry);
                }

                // Stream end of central directory (finish file)
                yield self.end_of_central_directory();

            }
                    
        }
        
    }

}