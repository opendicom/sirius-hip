# Dataset Overrides (metadata_overrides with `dataset=true`)

This document explains how **dataset overrides** work in Sirius HIP’s `metadata_overrides` configuration.

## What problem this solves

Some PACS installations store certain DICOM attributes **outside** the standard columns Sirius HIP normally reads (or store them in a custom binary blob). Dataset overrides let you:

- Describe **what** attribute you want to override (DICOM *keyword*).
- Describe **where** it comes from (a qualified `table.column`).
- Describe **how** it must be interpreted (`dataset=true` → the column contains a DICOM dataset blob).

This is especially useful when the PACS stores attributes in custom dataset columns and you want OHIF (or other consumers) to see consistent values.

## Configuration format

`metadata_overrides` is a TOML array of objects:

```toml
[dicomarchive]
metadata_overrides = [
  { keyword = "InstitutionName",        source = "study.study_custom1", dataset = true },
  { keyword = "ReferringPhysicianName", source = "study.ref_physician", dataset = false },
]
```

### Fields

- `keyword`
  - A DICOM **keyword** (ASCII identifier style; letters/digits/underscore).
  - Used as the logical “what” you are overriding.

- `source`
  - Must be `table.column`.
  - Treated as **identifiers**, not raw SQL.

- `dataset`
  - `false`: `source` is a normal column containing the final value.
  - `true`: `source` is a column containing a **DICOM dataset blob**, and Sirius HIP extracts the value for `keyword` by parsing that dataset.

## Runtime behavior (high level)

### 1) Startup validation (fail-fast)

On startup Sirius HIP validates `metadata_overrides`. Invalid configuration stops the server early (fail-fast).

Validation rules include:

- `keyword` must be a simple identifier.
- `source` must be valid `table.column` (both parts must be simple identifiers).
- No duplicate `keyword` entries.
- **Maximum 4 distinct dataset sources** (`dataset=true`).
  - This matches the current read-model slots `ov_ds1..ov_ds4`.

#### Dataset overrides cannot be used in SQL filtering

If an override is marked `dataset=true` for a keyword which is used in SQL `WHERE` filtering, Sirius HIP rejects the config at startup.

Reason: dataset blobs require parsing and cannot be used safely/efficiently as a SQL predicate.

### 2) Query-time selection of dataset blobs (StudyToken)

When StudyToken data is queried, Sirius HIP collects the set of distinct `source` values where `dataset=true`.

- The set is **deduplicated** and **sorted** for deterministic behavior.
- Up to 4 sources are selected into the SQL result as:
  - `ov_ds1`, `ov_ds2`, `ov_ds3`, `ov_ds4`

The mapping is positional:

- `ov_ds1` corresponds to the 1st dataset source in the sorted list
- `ov_ds2` to the 2nd, etc.

### 3) OHIF rendering: decoding the right dataset

The OHIF presenter always decodes `inst_attrs` (the normal instance dataset) because it is needed for required OHIF instance tags.

If an override exists with:

- `keyword = <OHIF-required instance keyword>`
- and `dataset = true`

then OHIF will try to read the attribute from the matching override dataset (`ov_dsN`) instead of `inst_attrs`.

Implementation details:

- The override dataset blob is decoded with:
  - `dicom_object::InMemDicomObject::read_dataset_with_ts(bytes, ts)`
- Decoding is cached **per row per dataset source** to avoid decoding the same blob multiple times.
- If the override dataset column is `NULL` for a specific row, Sirius HIP falls back to `inst_attrs`.

Currently, this dataset selection is implemented for these OHIF-required instance keywords:

- `Columns`
- `Rows`
- `PhotometricInterpretation`
- `BitsAllocated`
- `PlanarConfiguration`

## Safety / SQL injection considerations

- `source` is treated strictly as an identifier reference (`table.column`).
- Both `table` and `column` must match a restricted identifier pattern.
- The SQL expression is built as `table.column` (column name is backticked).

This avoids allowing arbitrary SQL expressions in config.

## Schema/version constraints (current behavior)

Dataset override sources must reference a table that is already joined by the StudyToken query for the selected PACS schema.

Fail-fast table allow-list for `dataset=true`:

- `dcm4chee2183`: `study`, `patient`, `series`, `instance`, `files`
- `dcm4chee440`: `study`, `patient`, `person_name`, `series`, `instance`, `file_ref`, `dicomattrs`

If you configure `dataset=true` for a table outside the allow-list, Sirius HIP fails at startup with a clear error.

## Limitations and planned extensions

- Only **4 distinct dataset sources** are supported right now (`ov_ds1..ov_ds4`).
  - This can be lifted by switching to a dynamic row model or a JSON/side-channel payload.

- Sirius HIP currently does **not** auto-add JOINs for dataset overrides.
  - If you need a dataset source from a new table, the safe approach is to explicitly add support in the repository implementation (or implement an explicit “join map” that enumerates permitted joins).

## Troubleshooting

### “Invalid metadata_overrides.source (expected table.column)”

- Ensure you used `table.column` (exactly one dot).
- Ensure both parts contain only letters/digits/underscore and don’t start with a digit.

### “too many distinct dataset sources”

- Reduce the number of distinct `source` values with `dataset=true` to 4 or fewer.

### “dataset=true … references table … not joined by the StudyToken query”

- Your `source` table is not available in the current StudyToken SQL.
- Either change `source` to a joined table or extend the repository query to include the required join.
