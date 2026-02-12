# Proprietary methods

- `GET` [`/echo`](#GET-/echo) Simple status check
- `GET` [`/settings`](#GET-/settings) Show runtime configuration
- `GET` [`/custodians/oids`](#GET-/custodians/oids) List all custodians oids configured
- `GET` [`/custodians/oids/{oid}/aeis`](#GET-/custodians/oids/{oid}/aeis) List all AETs configured in the specific custodian
- `GET` [`/pacs/{oid}/properties/stow`](#GET-/pacs/{oid}/properties/stow) Get stow url in the specific PACS
- `GET` [`/pacs/{oid}/properties/qido`](#GET-/pacs/{oid}/properties/qido) Get qido url in the specific PACS
- `GET` [`/pacs/{oid}/properties/wadouri`](#GET-/pacs/{oid}/properties/wadouri) Get wado url in the specific PACS
- `GET` [`/studyToken`](#GET-/studyToken) Construct manifest to view dicom studies in deferents viewers
- URL builder (Docker nginx): `/urlbuilder/study-token.html` HTML tool to build the url for *studyToken* method





##  `GET` `/echo`

Simple status check. Response with "echo" if the service is running

#### Response

- http code: `200`
- content-type: `text/plain`

```
echo
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip/echo
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## `GET` `/settings`

Show runtime configuration in json format. Password are hidden.

#### Response

- http code: `200`

- content-type: `application/json`

```json
{
	"database_url": "mysql://pacs:xxxxxxx@127.0.0.1:3306/pacsdb (password hidden)",
	"loglevel": "debug,hyper=info,reqwest=info,actix_web=info",
	"max_default": 2000,
	"database_max_connections": 40,
	"studytoken_exclude_mods": null,
	"jwt_auth": true,
	"jwt_secret": "******* (password hidden)",
	"jwt_algorithm": "HS256",
	"dicomarchive": {
		"custodianoid": "2.16.123.123.123",
		"pacsoid": "2.16.858.456.456.456",
		"pacsaet": "DCM4CHEE",
		"version": "dcm4chee2183",
		"wadouri": "http://localhost:8080/wado",
		"manifest_base_url": null,
		"transfer_syntax": "1.2.840.10008.1.2",
		"stow": null,
		"qido": null,
		"number_frames_field": null,
		"filesystems": [
			{
				"id": 1,
				"path": "/archive01"
			},
				"path": "/archive02"
			}
		]
	},
	"cors_whitelist": [
		"http://localhost:3000",
		"http://ohif.example.com:3000"
	]
     curl -X GET http://sirius-hip/echo
```
#### Example cURL

```bash
 curl -X GET http://sirius-hip/settings
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)


## `GET` `/custodians/oids`
List all custodians oids configured

#### Response

- http code: `200`

- content-type: `application/json`


```json
["2.16.123.123.123"]
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip/custodians/oids
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## `GET` `/custodians/oids/{oid}/aeis`

List all AETs configured in the specific custodian

#### Parameters

| name  | type | required | description                |
| ----- | ---- | -------- | -------------------------- |
| `oid` | Path | true     | The specific custodian OID |



#### Response

- http code: `200`

- content-type: `plain/text`

     curl -X GET http://sirius-hip/settings
```json
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip/custodians/oids/2.16.123.123.123/aeis
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## `GET` `/pacs/{oid}/properties/stow`

Get stow url in the specific PACS

#### Parameters

| name  | type | required | description           |
| ----- | ---- | -------- | --------------------- |
| `oid` | Path | true     | The specific PACS OID |



#### Response

- http code: `200`

- content-type: `plain/text`


```
http://pacs/stow
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip//pacs/2.16.858.456.456.456/properties/stow
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## `GET` `/pacs/{oid}/properties/qido`

Get qido url in the specific PACS

#### Parameters

| name  | type | required | description           |
| ----- | ---- | -------- | --------------------- |
| `oid` | Path | true     | The specific PACS OID |



#### Response

- http code: `200`

- content-type: `plain/text`

```json
http://pacs/qido
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip//pacs/2.16.858.456.456.456/properties/qido
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## `GET` `/pacs/{oid}/properties/wadouri`

Get wado uri in the specific PACS

#### Parameters

| name  | type | required | description           |
| ----- | ---- | -------- | --------------------- |
| `oid` | Path | true     | The specific PACS OID |



#### Response

- http code: `200`

- content-type: `plain/text`


```
http://pacs/wado
```

#### Example cURL

```bash
 curl -X GET http://sirius-hip//pacs/2.16.858.456.456.456/properties/wadouri
```

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## URL builder (Docker nginx): `/urlbuilder/study-token.html`

Construct manifest or dicom zip file to download or view dicom studies in diferents viewers:

- [cornerstone](https://cornerstonejs.org/)
- [ohif](https://viewer.ohif.org/)
- [weasis](https://weasis.org/en/index.html)

#### Parameters

| name                | type  | required              | description                                                  |
| ------------------- | ----- | --------------------- | ------------------------------------------------------------ |
| `accessType`        | query | required              | Type of response. Either a manifest to view dicom images from different viewers or a compressed file with the dicom images.</p>*Available values* : <br /><ul><li>cornerstone.json</li><li>weasis.xml</li><li>dicom.zip</li><li>ohif</li></ul> |
| `token`             | query | optional/required[^1] | Token for external sofware interoperability.<br />If Sirius HIP is configured to use *JWT Authorization*, this token must be compose by the external entity and shared to *Sirius HIP* in all the requests, *Sirius HIP* will response only if the token is valid.          <br />External entity must build this token as folow:<br />`{ "aud": "sirius-hip", "exp": TIMESTAMP }`<br /><br />- **Algorithm:** Same as Sirius HIP configuration<br />- **Secret key:** Same as Sirius HIP configuration |
| `session`           | query | optional              | Session token used for external sofware interoperability.<br />External entity session validation. This parameter is added in every response. So the calling entity can validate the session handed by it |
| `proxyURI`          | query | optional              | If configured, use this url as the base url for downloading the images from the manifest. Otherwise, use the Sirius HIP base URL |
| `AccessionNumber`   | query | optional              | Accession number *Dicom tag (0008,0050)* to search for       |
| `PatientID`         | query | optional              | Patient ID *Dicom tag (0010,0020)* to search for             |
| `patient`           | query | optional              | Patient Name, *Dicom Tag (0010,0010)* to search for          |
| `StudyInstanceUID`  | query | optional              | List of Studies instance UID *Dicom tag (0020,000D)* to search, \(back slash) separated |
| `StudyID`           | query | optional              | StudyID, *Dicom Tag (0020,0010)* to search for               |
| `StudyDate`         | query | optional              | Study date, *Dicom tag (0008,0020)*<br />*Allowed format: <br /><ul><li>`AAAA-MM-DD`</li><li>`AAAA-MM-DD|` (>=AAA-MM-DD)</li><li>`|AAAA-MM-DD` (<=AAA-MM-DD)</li><li>`AAAA-MM-DD|AAAA-MM-DD` (between)</li></ul> |
| `ModalityInStudy`   | query | optional              | Modality a study must contain                                |
| `cuidsInStudy`      | query | optional              | SOP Class OID in Study, *Dicom tag (0008,0016)* in study     |
| `SeriesInstanceUID` | query | optional              | List of Studies instance UID *Dicom tag (0020,000E)* to search, \(back slash) separated |
| `SeriesDescription` | query | optional              | Serie description *Dicom tag (0008,103E)* to search for      |
| `SeriesNumber`      | query | optional              | Serie number, *Dicom tag (0020,0011)* to search for          |
| `Modality`          | query | optional              | Modality of the Serie to search for                          |
| `ModalityOff`       | query | optional              | List of Modalities to exclude in te response, \(back slash) separated |
| `SOPClass`          | query | optional              | SOP Class UID of the Serie to search for                     |
| `SOPClassOff`       | query | optional              | List of Instance SOP Class UID to exclude in the response, \(back slash) separated |
| `institution`       | query | optional              | PACS OID.                                                    |
| `max`               | query | optional              | Maximum number of images query by request:<br /><br />Default: `2000` |

[^1]: Depends on configuration settings



#### Response

- http code: `200`

- content-type: `application/xml`

- accessType=weasis.xml


```xml
<?xml version="1.0" encoding="UTF-8"?>
<manifest
  xmlns="http://www.weasis.org/xsd/2.5"
  xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <arcQuery arcId="2.16.858.2.10002752.72769.3" baseUrl="http://localhost:6001">
    <Patient PatientID="33333333" PatientName="SIMPSON^BART" PatientSex="M" PatientBirthDate="19650523">
      <Study StudyInstanceUID="1.2.276.0.7230010.3.1.2.8323329.89819.1694438736.678182" StudyDescription="RX HOMBRO" StudyDate="20230908" StudyTime="145533.718" AccessionNumber="RX HOMBRO" StudyID="*" ReferringPhysicianName="RIVIERA^NICK">
        <Series SeriesInstanceUID="1.2.840.113564.1025239202.20230908145453984950" SeriesNumber="1" Modality="CR" SeriesDescription="AP">
          <Instance SOPInstanceUID="1.2.276.0.7230010.3.1.4.8323329.89850.1694438736.901296" InstanceNumber="1"/>
        </Series>
      </Study>
    </Patient>
  </arcQuery>
</manifest>
```

------

#### Response

- http code: `200`

- content-type: `application/json`

- accessType=ohif


```json
{
  "studies":[
    {
      "StudyInstanceUID":"1.2.276.0.7230010.3.1.2.8323329.89819.1694438736.678182",
      "StudyDate":"20230908",
      "StudyTime":"145533.718",
      "PatientName":"SIMPSON^BART",
      "PatientID":"33333333",
      "AccessionNumber":"CR230908145533.71826",
      "PatientAge":"58",
      "PatientSex":"M",
      "NumInstances":2,
      "Modalities":"OT\\CR",
      "series":[
        {
          "SeriesInstanceUID":"1.2.840.113564.1025239202.20230908145453984950",
          "SeriesNumber":1,
          "Modality":"CR",
          "instances":[
            {
              "metadata":{
                "InstanceNumber":1,
                "SOPClassUID":"1.2.840.10008.5.1.4.1.1.1",
                "Modality":"CR",
                "SOPInstanceUID":"1.2.276.0.7230010.3.1.4.8323329.89850.1694438736.901296",
                "SeriesInstanceUID":"1.2.840.113564.1025239202.20230908145453984950",
                "StudyInstanceUID":"1.2.276.0.7230010.3.1.2.8323329.89819.1694438736.678182",
                "SeriesDate":"20230911",
                "Columns":3020,
                "Rows":2400,
                "PhotometricInterpretation":"MONOCHROME2",
                "BitsAllocated":16
              },
              "url":"dicomweb:http://localhost:6001/wado?requestType=WADO&studyUID=1.2.276.0.7230010.3.1.2.8323329.89819.1694438736.678182&seriesUID=1.2.840.113564.1025239202.20230908145453984950&objectUID=1.2.276.0.7230010.3.1.4.8323329.89850.1694438736.901296&transferSyntax=1.2.840.10008.1.2&contentType=application/dicom&custodianOID=2.16.858.0.2.16.86.1.0.0.212701040013&arcId=2.16.858.2.10002752.72769.3"
            }
          ]
        }
      ]
    }
  ]
}
```

#### Response

- http code: `200`

- content-type: `application/octet-stream`

- accessType=dicom.zip

#### Example cURL

```bash
curl -X GET http://sirius-hip/studyToken?accessType=dicom.zip&StudyInstanceUID=1.2.276.0.7230010.3.1.2.8323329.89819.1694438736.678182
```



------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)



## URL builder (Docker nginx): `/urlbuilder/study-token.html`

HTML tool to build the url for *studyToken* method.

Note: this is served by nginx in the Docker image.

![resources/studytoken-urlbuilder-jpg](../resources/studytoken-urlbuilder.png)

------

[[Up]](#proprietary-methods)  [[Back]](README.md)  [[Start]](../../README.md)

