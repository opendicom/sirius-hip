use crate::settings::MetadataOverride;


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Represents the mode of metadata inclusion for study queries.
/// This enum is used to determine which metadata fields should be included in the query results based on the provided flags.
/// - `None`: No metadata fields are included.
/// - `Weasis`: Weasis-specific metadata fields are included.
/// - `Ohif`: OHIF-specific metadata fields are included.
pub enum MetadataMode {
    None,
    Weasis,
    Ohif,
}

/// Returns the appropriate `MetadataMode` based on the provided flags.
pub fn metadata_mode(include_ohif_metadata: bool, include_weasis_metadata: bool) -> MetadataMode {
    if include_ohif_metadata {
        MetadataMode::Ohif
    } else if include_weasis_metadata {
        MetadataMode::Weasis
    } else {
        MetadataMode::None
    }
}

/// Helper functions for constructing SQL expressions based on the `MetadataMode`.
pub fn include_patient_metadata(mode: MetadataMode) -> bool {
    mode != MetadataMode::None
}

/// Returns a SQL expression that selects patient metadata if `mode` indicates it should be included, or `NULL` otherwise.
pub fn select_patient_metadata(mode: MetadataMode, expr_if_included: &str) -> &str {
    if include_patient_metadata(mode) {
        expr_if_included
    } else {
        "NULL"
    }
}

/// Returns a SQL expression that selects patient metadata if `mode` indicates it should be included, or an alternative expression otherwise.
pub fn select_patient_metadata_else<'a>(
    mode: MetadataMode,
    expr_if_included: &'a str,
    expr_if_not_included: &'a str,
) -> &'a str {
    if include_patient_metadata(mode) {
        expr_if_included
    } else {
        expr_if_not_included
    }
}

/// Returns a SQL expression that selects the given expression if `mode` is not `None`, or `NULL` otherwise.
pub fn select_non_none(mode: MetadataMode, expr_if_included: &str) -> &str {
    if mode != MetadataMode::None {
        expr_if_included
    } else {
        "NULL"
    }
}

/// Returns a SQL expression that selects the given expression if `mode` is `Ohif`, or `NULL` otherwise.
pub fn select_ohif_only(mode: MetadataMode, expr_if_ohif: &str) -> &str {
    if mode == MetadataMode::Ohif {
        expr_if_ohif
    } else {
        "NULL"
    }
}

/// Returns a SQL expression that selects the appropriate expression based on the `MetadataMode`.
pub fn select_mode3<'a>(mode: MetadataMode, expr_none: &'a str, expr_weasis: &'a str, expr_ohif: &'a str) -> &'a str {
    match mode {
        MetadataMode::None => expr_none,
        MetadataMode::Weasis => expr_weasis,
        MetadataMode::Ohif => expr_ohif,
    }
}

/// Validates that a string is a simple SQL identifier (consisting of ASCII letters, digits, or underscores, and not starting with a digit).
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

/// Parses an override reference in the format `table.column` and validates that both parts are simple identifiers.
/// Returns `Some((table, column))` if valid, or `None` if the
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
