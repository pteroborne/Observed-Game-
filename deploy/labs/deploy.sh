#!/usr/bin/env bash
set -euo pipefail

deployment_dir="${OBSERVED_LABS_DIR:-/opt/observed-labs}"
compose_file="${OBSERVED_LABS_COMPOSE_FILE:-${deployment_dir}/compose.yaml}"
project_name="observed-labs"

if [[ ! -f "$compose_file" ]]; then
  echo "Missing Compose file: $compose_file" >&2
  exit 1
fi

compose=(docker compose --project-name "$project_name" --file "$compose_file")
container_id="$("${compose[@]}" ps --quiet observed-labs 2>/dev/null || true)"
previous_image=""
if [[ -n "$container_id" ]]; then
  previous_image="$(docker inspect --format '{{.Image}}' "$container_id")"
fi

"${compose[@]}" pull observed-labs
"${compose[@]}" up --detach --remove-orphans observed-labs

container_id="$("${compose[@]}" ps --quiet observed-labs)"
if [[ -z "$container_id" ]]; then
  echo "Compose did not create the observed-labs container." >&2
  exit 1
fi

for _ in {1..24}; do
  health="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id")"
  case "$health" in
    healthy)
      current_image="$(docker inspect --format '{{.Image}}' "$container_id")"
      if [[ "$current_image" == "$previous_image" ]]; then
        echo "Observed labs are healthy; the deployed image is unchanged."
      else
        echo "Observed labs updated to image $current_image and are healthy."
      fi
      exit 0
      ;;
    unhealthy|exited|dead)
      "${compose[@]}" logs --tail 100 observed-labs >&2
      exit 1
      ;;
  esac
  sleep 5
done

echo "Observed labs did not become healthy within 120 seconds." >&2
"${compose[@]}" logs --tail 100 observed-labs >&2
exit 1
