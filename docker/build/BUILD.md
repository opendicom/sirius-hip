```bash
cd HOME_REPO
docker build -f docker/build/Dockerfile_rocky8 -t opendicom/sirius-hip:latest-r8 .
docker build -f docker/build/Dockerfile -t opendicom/sirius-hip:latest .
```