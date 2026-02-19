# Build Docker image
```bash
cd <HOME_REPO>
docker build -f docker/build/Dockerfile_rocky8 -t opendicom/sirius-hip:latest-r8 .
docker build -f docker/build/Dockerfile -t opendicom/sirius-hip:latest .
```


# Extract bin
```bash
mkdir -p dist/rocky8/
docker run --name sirius-hip-rocky8 opendicom/sirius-hip:latest-r8
docker cp sirius-hip-rocky8:/root/.cargo/bin/sirius-hip dist/rocky8/
docker stop sirius-hip-rocky8
docker rm sirius-hip-rocky8
docker image rm opendicom/sirius-hip:latest-r8
```
