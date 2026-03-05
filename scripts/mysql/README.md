# MySQL scripts

This folder contains manual SQL scripts to set up PACS-side helpers used by Sirius HIP.

## Dirty study triggers

These scripts create a small table `HIP_dirty_study` and define `AFTER UPDATE` triggers to keep it up to date.

- dcm4chee 2.18.3: `dcm4chee2183_dirty_triggers.sql`
- dcm4chee 4.4.0: `dcm4chee440_dirty_triggers.sql`

### What they do

On any `UPDATE` to `study`, `series`, or `instance`, the corresponding study is upserted into `HIP_dirty_study`:

- `study_iuid`: StudyInstanceUID
- `dirty_since`: first time we marked the study dirty (UTC)
- `last_dirty_at`: last time we re-marked it dirty (UTC)
- `reason`: currently `update`
- `source_table`: one of `study|series|instance`

This is intentionally **sticky**: once dirty, later ingestion (new series/instances) does not clear it.

These scripts are also intentionally **strict**: they only mark dirty when meaningful metadata fields change
(e.g. descriptions, *_attrs blobs, `dicomattrs_fk`), not when only counters or `updated_time` are updated.

### How to run

Pick the script matching your PACS DB schema and run it against the PACS database:

- `mysql -h <host> -u <user> -p <db> < scripts/mysql/dcm4chee2183_dirty_triggers.sql`
- `mysql -h <host> -u <user> -p <db> < scripts/mysql/dcm4chee440_dirty_triggers.sql`

### Privileges

The MySQL user must have at least:

- `CREATE` (table)
- `TRIGGER`

### Tuning

These triggers are conservative (any UPDATE marks dirty). If you want a narrower definition
(e.g., only when attrs/desc change but not counter updates), we can refine predicates per table.
