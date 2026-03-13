use std::collections::HashSet;
use std::fmt::Display;
use std::fmt::Write;

use anyhow::bail;
use serde::{Serialize, Deserialize};
use sqlx::Arguments;
use sqlx::mysql::MySql;
use sqlx::mysql::MySqlArguments;
use sqlx::mysql::MySqlPool;
use sqlx::encode::Encode;
use sqlx::types::Type;

use crate::api::qido::QidoStudiesParams;
use crate::models::qido::Qido;
use crate::{api::study_token::params::StudyTokenParams, 
            settings::Settings, 
            models::weasis, 
            models::ohif,
            models::cornerstone};

mod dcm4chee440;
mod dcm4chee2183;

pub mod helpers;


// --------------------------------------------------------- //
// --- Supported database versions ------------------------- //
// --------------------------------------------------------- //

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")] 
#[allow(non_camel_case_types)]
pub enum DBVersion {
    dcm4chee2183,
    dcm4chee440
}


// --------------------------------------------------------- //
// -- WEASIS static database module dispatcher ------------- //
// --------------------------------------------------------- //

/// Dispatch query based on `settings.version.version` value
pub async fn get_weasis_studies(
    pool: &MySqlPool, 
    params: &StudyTokenParams, 
    settings: &Settings) 
    -> anyhow::Result<weasis::Studies> 
{
    match settings.dicomarchive.version {
        DBVersion::dcm4chee2183 => dcm4chee2183::weasis::get_studies(pool, params, settings).await,
        DBVersion::dcm4chee440 =>  dcm4chee440::weasis::get_studies(pool, params, settings).await,
    }
}

// --------------------------------------------------------- //
// -- OHIF static database module dispatcher --------------- //
// --------------------------------------------------------- //

/// Dispatch query based on `settings.version.version` value
pub async fn get_ohif_studies(
    pool: &MySqlPool, 
    params: &StudyTokenParams, 
    settings: &Settings, 
    server_base_url: String) 
    -> anyhow::Result<ohif::Studies> 
{
    match settings.dicomarchive.version {
        DBVersion::dcm4chee2183 => dcm4chee2183::ohif::get_studies(pool, params, settings, server_base_url).await,
        DBVersion::dcm4chee440 =>  dcm4chee440::ohif::get_studies(pool, params, settings, server_base_url).await,
    }    
}


// --------------------------------------------------------- //
// -- CORNERSTONE static database module dispatcher -------- //
// --------------------------------------------------------- //

/// Dispatch query based on `settings.version.version` value
pub async fn get_cornerstone_studies(
    pool: &MySqlPool, 
    params: &StudyTokenParams, 
    settings: &Settings)
    -> anyhow::Result<cornerstone::Studies> 
{
    match settings.dicomarchive.version {
        DBVersion::dcm4chee2183 => dcm4chee2183::cornerstone::get_studies(pool, params, settings).await,
        DBVersion::dcm4chee440 =>  bail!("NOT IMPLEMENTED"),
    }    
}

// --------------------------------------------------------- //
// -- QIDO static database module dispatcher -------- //
// --------------------------------------------------------- //

/// Dispatch query based on `settings.version.version` value
pub async fn get_qido_studies(
    pool: &MySqlPool,
    validated_include_fields: HashSet<&'static str>, 
    params: &QidoStudiesParams,
    settings: &Settings)
    -> anyhow::Result<Qido> 
{
    match settings.dicomarchive.version {
        DBVersion::dcm4chee2183 => bail!("NOT IMPLEMENTED"),
        DBVersion::dcm4chee440 =>  dcm4chee440::qido::get_studies(pool, params, validated_include_fields, settings).await,
    }    
}

// --------------------------------------------------------- //
// -- QueryBuilder Implementation -------------------------- //
// --------------------------------------------------------- //

#[derive(Default)]
/// Create a `sqlx::Query` in run-time
pub struct QueryBuilder {
    // Probably lots of optional fields.
    select: String,
    from: String,
    condition: String,
    order: String,
    limit: u64,
    query: String,
    arguments: Option<MySqlArguments>
}

impl QueryBuilder {
    pub fn new() -> QueryBuilder {
        QueryBuilder {
            select: String::new(),
            from: String::new(),
            condition: String::new(),
            order: String::new(),
            limit: 0,
            query: String::new(),
            arguments: Some(MySqlArguments::default()),
        }
    }

    pub fn select(&mut self, fields: impl Display) -> &mut Self {

        if self.select.len() ==  0 {
            write!(self.select, "SELECT {}", fields).expect("error formatting `sql`");
        } else {
            write!(self.select, ", {}", fields).expect("error formatting `sql`");
        }
        if self.select.ends_with(',') {
            self.select.truncate(self.select.len()-2);
        }
        self
    }

    // pub fn select_opt<T>(&mut self, fields: Option<T>) -> &mut Self 
    // where T: Display {

    //     if let Some(value) = fields {
    //         if self.select.len() ==  0 {
    //             write!(self.select, "SELECT {}", value).expect("error formatting `sql`");
    //         } else {
    //             write!(self.select, ", {}", value).expect("error formatting `sql`");
    //         }
    //         if self.select.ends_with(',') {
    //             self.select.truncate(self.select.len()-2);
    //         }
    //     }
    //     self
    // }


    pub fn from(&mut self, sql: impl Display) -> &mut Self {

        if self.from.len() == 0 {
            write!(self.from, " FROM {}", sql).expect("error formatting `sql`");
        } else {
            write!(self.from, " {}", sql).expect("error formatting `sql`");
        }
        self
    }


    pub fn condition<'args, T>(&mut self, sql: impl Display, value: T ) -> &mut QueryBuilder  
    where 
        T: 'args + Encode<'args, MySql> + Send + Type<MySql> {
        
        if self.condition.len() == 0 {
            write!(self.condition, " WHERE {}", sql).expect("error formatting `sql`");
        } else {
            write!(self.condition, " AND {}", sql).expect("error formatting `sql`");
        }
        if let Some(args) = &mut self.arguments {
            args.add(value);
        }
        self
    }

    pub fn condition_between<'args, T>(&mut self, sql: impl Display, start: T, end: T ) -> &mut QueryBuilder  
    where 
        T: 'args + Encode<'args, MySql> + Send + Type<MySql> {
        
        if self.condition.len() == 0 {
            write!(self.condition, " WHERE {}", sql).expect("error formatting `sql`");
        } else {
            write!(self.condition, " AND {}", sql).expect("error formatting `sql`");
        }
        if let Some(args) = &mut self.arguments {
            args.add(start);
            args.add(end);
        }
        self
    }


    pub fn condition_opt<'args, T>(&mut self, sql: impl Display, value: Option<T> ) -> &mut QueryBuilder  
    where 
        T: 'args + Encode<'args, MySql> + Send + Type<MySql> {
        
        if let Some(value) = value {
            if self.condition.len() == 0 {
                write!(self.condition, " WHERE {}", sql).expect("error formatting `sql`");
            } else {
                write!(self.condition, " AND {}", sql).expect("error formatting `sql`");
            }
            if let Some(args) = &mut self.arguments {
                args.add(value);
            } 
        }
        self
    }

    // TODO
    pub fn condition_list_opt(&mut self, sql: impl Display, value: Option<&String>, delimiter: char ) -> &mut QueryBuilder {
        
        if let Some(value) = value {
            if self.condition.len() == 0 {
                write!(self.condition, " WHERE {} (", sql).expect("error formatting `sql`");
            } else {
                write!(self.condition, " AND {} (", sql).expect("error formatting `sql`");
            }

            for val in value.split(delimiter) {
                write!(self.condition, "? ,").expect("error formatting `sql`");
                if let Some(args) = &mut self.arguments {
                    args.add(val);
                }
            }
            self.condition.pop();
            self.condition.push(')');
        
        }
        self
    }

    pub fn condition_push(&mut self, sql: impl Display) -> &mut QueryBuilder {
        if self.condition.len() == 0 {
            write!(self.condition, " WHERE {}", sql).expect("error formatting `sql`");
        } else {
            write!(self.condition, " {}", sql).expect("error formatting `sql`");
        }
        self
    }


    pub fn bind<'args, T>(&mut self, value: T ) -> &mut QueryBuilder  
    where 
        T: 'args + Encode<'args, MySql> + Send + Type<MySql> {
        
        if let Some(args) = self.arguments.as_mut(){
            args.add(value)
        }
        self
    }


    pub fn order_by(&mut self, sql: impl Display) -> &mut Self {

        if self.order.len() >  0 {
            self.order.clear();
        }
        write!(self.order, " ORDER BY {}", sql).expect("error formatting `sql`");
        self
    }

    pub fn sql(&mut self) -> String {
        format!("{}{}{}{}{}",
            self.select,
            self.from,
            self.condition,
            self.order, 
            if self.limit > 0 {
                format!(" LIMIT {}",self.limit)
            }else {
                "".to_string()
            },
        )
    }
    

    pub fn build(&mut self) -> sqlx::query::Query<'_, MySql, MySqlArguments> {
        self.query = self.sql();
        sqlx::query_with(&self.query, self.arguments.take().unwrap())
    }

    pub fn limit(&mut self, value: u64) {
        self.limit = value;
    }

}