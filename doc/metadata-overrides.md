# Metadata Overrides (`metadata_overrides`)

Sirius HIP can map selected DICOM keywords to custom database columns.
This is useful when your PACS stores a value in a non-standard column, or when you want a different field to be exposed through Sirius HIP.

## Configuration

`metadata_overrides` is a TOML array of objects.

Example:

```toml
metadata_overrides = [
  { keyword = "PatientID",              source = "patient.pat_id" },
  { keyword = "StudyDescription",       source = "study.study_custom1" },
  { keyword = "ReferringPhysicianName", source = "study.ref_physician" },
]
```

Fields:

- `keyword`: a DICOM keyword (ASCII letters/digits/underscore, starting with a letter/underscore).
- `source`: a qualified identifier in the form `table.column`.

## Validation rules

At startup Sirius HIP validates:

- `keyword` is a simple identifier (safe to use in internal SQL aliasing).
- `source` matches `table.column`, and both parts are simple identifiers.
- keywords are unique.

Invalid configuration fails fast and prevents the server from starting.

## Notes

- Overrides only affect values that Sirius HIP reads from the database; they do not add joins automatically. The `table` you reference must already be available in the query built by the corresponding repository implementation.
