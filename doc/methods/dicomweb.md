# DICOMweb-style methods

Sirius HIP exposes a subset of DICOMweb-style endpoints (Part 18). The main supported workflow today is **QIDO-RS SearchForStudies**.

Reference: [QIDO-RS DICOMweb Services](http://dicom.nema.org/dicom/2013/output/chtml/part18/sect_6.7.html)

## QIDO

### `GET /qido/studies`

Search for studies.

Notes:

- The handler currently expects `content-type: application/json` (even though the request is a GET).
- `/qido/series` and `/qido/instances` exist in routing but are not implemented in the src2 flow yet.

#### Example

```bash
curl -G "http://localhost:5001/qido/studies" \
	-H "content-type: application/json" \
	--data-urlencode "PatientID=123" \
	--data-urlencode "limit=50"
```

#### Include extra fields

```bash
curl -G "http://localhost:5001/qido/studies" \
	-H "content-type: application/json" \
	--data-urlencode "PatientID=123" \
	--data-urlencode "includefield=StudyDescription" \
	--data-urlencode "includefield=SOPClassesInStudy"
```

For the full implementation details, supported query parameters, and known limitations, see:

- [doc/qido.md](../qido.md)

------

[[Back]](README.md)  [[Start]](../../README.md)
