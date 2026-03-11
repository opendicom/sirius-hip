# TODO List

- In conf validate usage of `institution_field` and `number_frames_field` 
and move it to override_datasets if applied.

- Weasis fail to open manifest when the requested data to /studyToken contains "|" or "\"
    - Error: SyntaxError: bad expression:"
    - Examples:
        - StudyDate=20260101|20260102       --> FAIL
        - StudyInstanceUID=1.2.3.4\5.6.7.8  --> FAIL