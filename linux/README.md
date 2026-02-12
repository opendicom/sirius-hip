# Install Sirius HIP under Linux

1. Copy `sirius-hip.service` under `/etc/systemd/system/`
2. Copy `sirius-hip.rsyslog.conf` under `/etc/rsyslog.d/`
3. Run `systemctl daemon-reload && systemctl restart rsyslog.service`
