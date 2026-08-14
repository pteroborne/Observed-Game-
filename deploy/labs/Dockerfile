FROM nginx:1.27.5-alpine@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10

ARG BUILD_REVISION=unknown
ARG BUILD_VERSION=dev

LABEL org.opencontainers.image.title="Observed Labs" \
      org.opencontainers.image.description="Browser-hosted Observed 2 feasibility labs" \
      org.opencontainers.image.source="https://github.com/pteroborne/Observed-Game-" \
      org.opencontainers.image.revision="$BUILD_REVISION" \
      org.opencontainers.image.version="$BUILD_VERSION"

RUN rm -rf /usr/share/nginx/html/*

COPY deploy/labs/nginx.conf /etc/nginx/conf.d/default.conf
COPY deploy/labs/site/index.html /usr/share/nginx/html/index.html
COPY web-dist/tactics-lab/ /usr/share/nginx/html/tactics/

# Nginx serves the precompressed files when the browser advertises gzip. The
# original files remain available to clients that do not.
RUN find /usr/share/nginx/html -type f \( -name '*.js' -o -name '*.wasm' \) \
      -exec gzip -9 -k -f '{}' \;

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
