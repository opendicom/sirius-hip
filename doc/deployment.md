## Deployment

This repository ships a ready-to-run Docker image and a reference docker-compose stack.

### Docker image

Sirius HIP is published as a Docker image:

- https://hub.docker.com/r/opendicom/sirius-hip

### Run with Docker Compose (recommended)

The reference stack (PACS + MySQL + OHIF + Sirius HIP) lives under [docker/](../docker/).

1. Go to the docker folder:

   ```bash
   cd docker
   ```

2. Review and edit environment variables:

   - File: [docker/docker-compose.env](../docker/docker-compose.env)
   - Overrides:
     - Dev: [docker/docker-compose.dev.env](../docker/docker-compose.dev.env)
     - Prod: [docker/docker-compose.prod.env](../docker/docker-compose.prod.env)
   - Sirius HIP variables use the `SIRIUS_HIP_*` prefix.

   Important values:
   - `SIRIUS_HIP_PACS_VERSION`: `dcm4chee2183` or `dcm4chee440`
   - `SIRIUS_HIP_JWT_AUTH`: `standard` or `onetime`
   - `SIRIUS_HIP_FS_MAPPINGS`: filesystem mappings used for fast-path file reads

3. Start the stack:

   ```bash
   docker compose up -d
   ```

4. Verify the service:

   ```bash
   curl http://localhost:5001/echo
   ```

### Build a custom Docker image

1. Clone this repository

   ```bash
   git clone http://gitea.opendicom.com:8080/desarrollo/sirius-hip/
   ```

2. Build using the Dockerfile:

   ```bash
   docker build -t opendicom/sirius-hip:local -f docker/build/Dockerfile .
   ```

### Local run (Rust)

1. Install Rust (stable toolchain).
2. Prepare a config file (example: [sirius-hip.toml](../sirius-hip.toml)).
3. Run:

```bash
cargo run --release -- -c ./sirius-hip.toml
```

The server binds by default to `0.0.0.0:5001` (can be changed with `--bind`).

------

[[Back]](../README.md)

