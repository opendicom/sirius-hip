use regex::Regex;
use serde::{ser::Error, Serializer};

mod patient;
pub use patient::calculate_age;


/// Serialize a URL hiding the password part
pub fn url_password_hidden<S>(url: &str, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{   
    let re = Regex::new(r"(mysql://.*:).*(@.*)").map_err(Error::custom)?;
    let hidden = re.replace(url, "${1}xxxxxxx${2} (password hidden)");

    s.serialize_str(&hidden)
}

/// Serialize a password hiding its actual value
pub fn password_hidden<S>(_: &str, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{   
    s.serialize_str("******* (password hidden)")
}



