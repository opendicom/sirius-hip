use crate::settings::MetadataOverride;

pub fn is_simple_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_override_ref(value: &str) -> Option<(&str, &str)> {
    let (table, col) = value.split_once('.')?;
    if table.is_empty() || col.is_empty() {
        return None;
    }
    if !is_simple_identifier(table) || !is_simple_identifier(col) {
        return None;
    }
    Some((table, col))
}

/// Returns a qualified column expression like `alias.col` if the override exists and is safe.
///
/// `dicom_keyword` is the config key (e.g. "PatientID").
///
/// The override value MUST be `alias.column_name`.
/// (No backwards compatibility for the old `column_name`-only format.)
///
/// Both identifiers are validated as simple identifiers to avoid SQL injection.
pub fn override_col(
    overrides: Option<&[MetadataOverride]>,
    dicom_keyword: &str,
) -> Option<String> {
    let list = overrides?;
    let ov = list.iter().find(|ov| ov.keyword == dicom_keyword)?;
    let (table, col) = parse_override_ref(&ov.source)?;
    Some(format!("{}.`{}`", table, col))
}

/// Convenience helper returning either the override qualified column (if present/safe)
/// or the provided default SQL expression.
pub fn override_or_default(
    overrides: Option<&[MetadataOverride]>,
    dicom_keyword: &str,
    default_expr: &str,
) -> String {
    override_col(overrides, dicom_keyword).unwrap_or_else(|| default_expr.to_string())
}
