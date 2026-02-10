use crate::api::study_token::params::StudyTokenParams;
use crate::database::QueryBuilder;
use crate::settings::Settings;

pub mod weasis;
pub mod ohif;
pub mod dicomzip;
pub mod cornerstone;

impl StudyTokenParams {
    pub fn add_query_conditions(& self, query_builder: &mut QueryBuilder, settings: &Settings) {
        
        if let Some(value) = &self.study_date {
            // AAAA-MM-DD|  (equal or newer than AAAA-MM-DD)
            if value.ends_with('|') {
                query_builder.condition("study.study_datetime >= ?", value.trim_end_matches('|'));
            }
            
            // |AAAA-MM-DD  (equal or older than AAAA-MM-DD)
            else if value.starts_with('|') {
                query_builder.condition("study.study_datetime <= ?", value.trim_start_matches('|'));
            }

            // AAAA-MM-DD|AAAA-DD-MM  (between)
            else if value.contains('|'){
                if let Some((start,end)) = value.split_once('|') {
                    query_builder.condition_between("study.study_datetime BETWEEN", start, end);
                }
            }
            // AAAA-MM-DD (equal)
            else {
                query_builder.condition("DATE(study.study_datetime) = ?", value);
            }
        }

        if let Some(field) = &settings.dicomarchive.institution_field {
            query_builder
                .select(format!("study.{field} as institution"))
                .condition_opt(format!("study.{field} = ?"), self.institution.as_ref());
        }

        query_builder
            .condition_opt("patient.pat_id = ?", self.patient_id.as_ref())
            .condition_opt("patient.pat_name REGEXP ?", self.patient_fullname.as_ref())
            .condition_opt("study.study_id LIKE %?%", self.study_id.as_ref())
            .condition_opt("study.mods_in_study LIKE %?%", self.modality_in_study.as_ref())
            .condition_list_opt("study.study_iuid IN" , self.study_instance_uid.as_ref(), '\\')
            .condition_opt("study.accession_no = ?", self.accession_number.as_ref())
            .condition_list_opt("series.series_iuid IN", self.series_instance_uid.as_ref(), '\\')
            .condition_opt("series.series_no = ?", self.series_number.as_ref())
            .condition_opt("series.series_desc LIKE %?%", self.series_description.as_ref())
            .condition_opt("series.modality = ?", self.modality.as_ref())
            .condition_opt("instance.sop_cuid = ?", self.sop_class.as_ref())
            .condition_opt("instance.sop_cuid != ?", self.sop_class_off.as_ref())
            .condition_list_opt("series.modality NOT IN ", self.modality_off.as_ref(), '\\')
            .limit(self.max.unwrap_or(settings.max_default));

    }
}
