#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "Run this installer with sudo." >&2
  exit 1
fi

for command_name in docker systemctl install; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Required command is missing: $command_name" >&2
    exit 1
  fi
done

if ! docker compose version >/dev/null 2>&1; then
  echo "Docker Compose v2 is required (the 'docker compose' command)." >&2
  exit 1
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
deployment_dir="/opt/observed-labs"

install -d -m 0755 "$deployment_dir"
install -m 0644 "$script_dir/compose.yaml" "$deployment_dir/compose.yaml"
install -m 0755 "$script_dir/deploy.sh" "$deployment_dir/deploy.sh"
install -m 0644 "$script_dir/observed-labs-deploy.service" /etc/systemd/system/observed-labs-deploy.service
install -m 0644 "$script_dir/observed-labs-deploy.timer" /etc/systemd/system/observed-labs-deploy.timer

if [[ ! -f "$deployment_dir/.env" ]]; then
  install -m 0600 /dev/null "$deployment_dir/.env"
  {
    echo "LAB_IMAGE=ghcr.io/pteroborne/observed-labs:edge"
    echo "LAB_BIND_ADDRESS=0.0.0.0"
    echo "LAB_HTTP_PORT=8086"
  } > "$deployment_dir/.env"
fi

systemctl daemon-reload
systemctl enable --now observed-labs-deploy.timer
systemctl start observed-labs-deploy.service

echo "Observed labs deployment is active."
echo "Direct LAN URL: http://192.168.1.136:8086/tactics/"
echo "Timer status: systemctl status observed-labs-deploy.timer"
echo "Deploy logs:  journalctl -u observed-labs-deploy.service"
