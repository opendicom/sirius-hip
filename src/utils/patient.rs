use chrono::{NaiveDate, Utc};

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