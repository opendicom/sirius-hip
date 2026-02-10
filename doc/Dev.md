#Command to develop
```
cargo-watch -w src -x 'run -- -b "0.0.0.0:5001" -c sirius-hip.dev.toml'
```

# Dicom ZIP
Crear archivo zip con data descriptor sin compresion 
```
zip -0 -fd sample.zip DICOM0001.dcm
zip -0 -fd sample_two_files.zip DICOM0002.dcm DICOM0001.dcm
```
En linux luego es posible renombrar el nombre del archvio desde el "Gestor de Archivadores"


osirix://?methodName=downloadURL&URL='...'