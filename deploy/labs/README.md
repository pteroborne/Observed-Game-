# Observed browser lab hosting

This directory turns the browser labs into one versioned static nginx image.
The current image exposes:

- `/` — a mobile-friendly lab index.
- `/tactics/` — the isometric tactics lab.
- `/composition/` — the composition studio, as a read-only viewer. The browser
  build has no corpus to write back to, so saving and promotion are absent
  rather than merely disabled.
- `/healthz` — a container and reverse-proxy health probe.

## Delivery model

1. Pull requests build and test the tactics lab on a GitHub-hosted runner.
2. A push to `main` publishes both an immutable `sha-<commit>` image and the
   moving `edge` channel to `ghcr.io/pteroborne/observed-labs`.
3. The Ubuntu host checks `edge` every two minutes and lets Docker Compose
   recreate the service only when its image changes.
4. The existing nginx/openresty instance proxies browser traffic to port 8086.

The host does not accept GitHub webhooks, expose Portainer, or run repository
code in a self-hosted Actions runner. That boundary matters because this is a
public repository. Portainer will still display and monitor the Compose-created
container as an externally managed stack.

## First installation on `192.168.1.136`

Prerequisites are Docker Engine, Docker Compose v2, and systemd. After this
change reaches `main` and the `Browser labs` workflow publishes its first image:

1. In the GitHub package settings for `observed-labs`, set the package visibility
   to **Public**. The deployed lab is intentionally LAN-visible and contains no
   private runtime data. If the package remains private, run `sudo docker login
   ghcr.io` on the server with a read-packages token before installing.
2. Clone or update this repository on the Ubuntu host.
3. From the repository root, run:

   ```bash
   sudo bash deploy/labs/install-host.sh
   ```

4. Verify the direct endpoint from another LAN device:

   ```text
   http://192.168.1.136:8086/tactics/
   ```

The installer copies only the Compose/deployment files to
`/opt/observed-labs`, preserves an existing `.env`, enables the polling timer,
and performs the first health-checked deployment.

## nginx / openresty

If nginx runs directly on the host, include
`nginx-location.conf.example` in the desired virtual host and reload nginx. The
public paths will be `/labs/tactics/` and `/labs/composition/`.

Keep the `proxy_redirect` line from that example. The container is mounted under
`/labs` but is not aware of it, so its trailing-slash redirects come back
root-relative and would otherwise send a browser that asked for
`/labs/composition` to `/composition/` on the public host.

For Nginx Proxy Manager, create a Proxy Host with scheme `http`, forward host
`192.168.1.136`, and forward port `8086`. The lab remains `/tactics/` on that
host. Keep Portainer's ports private; neither the CI nor the lab needs them.

The default Compose bind is `0.0.0.0:8086` so a phone can test it directly and
a containerized reverse proxy can reach it. If nginx is native and direct LAN
access is unwanted, set `LAB_BIND_ADDRESS=127.0.0.1` in
`/opt/observed-labs/.env`, then run the deploy service once.

## Operations and rollback

```bash
# Check the polling schedule and latest deployment result.
systemctl status observed-labs-deploy.timer
journalctl -u observed-labs-deploy.service -n 100

# Deploy immediately rather than waiting for the timer.
sudo systemctl start observed-labs-deploy.service

# Inspect the service through Compose or Portainer.
sudo docker compose -p observed-labs -f /opt/observed-labs/compose.yaml ps
```

For a rollback, replace the `LAB_IMAGE` value in `/opt/observed-labs/.env` with
an immutable tag such as
`ghcr.io/pteroborne/observed-labs:sha-<full-commit-sha>`, then start the deploy
service. Change it back to `:edge` to resume automatic updates.

## Adding another browser lab

Keep each lab at its own stable path inside the same image. Add its web build to
the workflow, copy its distribution into `/usr/share/nginx/html/<lab-name>/` in
the Dockerfile, and add a card to `site/index.html`. The host deployment and
reverse proxy do not need another port or pipeline.
