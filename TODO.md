# TODO List

- In conf validate usage of `institution_field` and `number_frames_field` 
and move it to override_datasets if applied.

- Change FS vs WADO selection for Study
    Prefer WADO when study.updated_time + WINDOW_TIME is newer than the updated_time of any of the study’s Series.