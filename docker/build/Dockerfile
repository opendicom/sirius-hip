FROM rust:1.81-bookworm AS builder
RUN apt install -y libssl-dev && \
    apt clean && \
    rm -rf /var/lib/apt/lists/* 
    
WORKDIR /usr/src/sirius-hip
COPY . .
RUN mkdir -p /usr/src/sirius-hip && \
    cargo install --path .

FROM debian:bookworm
RUN apt update && \
    apt install -y gettext curl nginx && \
    apt clean && \
    rm -rf /var/lib/apt/lists/* &&\
    mkdir -p /etc/sirius-hip/ && \
    mkdir /template && \
    mkdir /DICOM

WORKDIR /
COPY ./docker/build/docker-entrypoint.sh /
COPY ./docker/build/sirius-hip.conf.template /template/
COPY --from=builder /usr/local/cargo/bin/sirius-hip /usr/local/bin/sirius-hip
COPY --from=builder /usr/src/sirius-hip/www/studyToken/urlbuilder /var/www/html/

RUN sed -i 's/listen 80 default_server;/listen 5002 default_server;/g' /etc/nginx/sites-enabled/default
RUN sed -i 's/listen [::]:5002 default_server;/listen [::]:5002 default_server;/g' /etc/nginx/sites-enabled/default

HEALTHCHECK --interval=30s --timeout=10s --retries=10 --start-period=10s \
    CMD curl --silent --fail http://localhost:5001/echo || exit 1

EXPOSE 5001
EXPOSE 5002

ENTRYPOINT [ "/docker-entrypoint.sh" ]
CMD ["sirius-hip","-b", "0.0.0.0:5001","-c" ,"/etc/sirius-hip/sirius-hip.toml"]