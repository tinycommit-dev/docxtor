# syntax=docker/dockerfile:1
# docxtor-web: the Docxtor web app on an unprivileged nginx, port 8080. The same build as the hosted app and the self-host archive
# (scripts/build-web.mjs: the engine's shipped profile, wasm:ship, then the app).
#
#   docker build -f deploy/Dockerfile -t docxtor-web .        (from the repository root: the context is the repository, trimmed by .dockerignore)
#   docker run --rm -p 8080:8080 docxtor-web
#   docker run --rm -p 8080:8080 --read-only --tmpfs /tmp docxtor-web     (nothing is written outside /tmp)
#
# This file is in deploy/ with the rest of what serves the app (_headers, nginx.conf, Caddyfile); the build context is still the root.
# Base images are pinned by digest, the tag they were taken from is in the comment. To move one: docker pull it, read the digest with
# `docker images --digests`, and change both.

# --- build: Rust (for the engine) and Node (for the app) -----------------------------------------------------------------------------
# rust:1.92-trixie (Debian 13: wasm-bindgen's prebuilt program needs glibc 2.38, which Debian 12 lacks)
FROM rust:1.92-trixie@sha256:f58923369ba295ae1f60bc49d03f2c955a5c93a0b7d49acfb2b2a65bebaf350d AS build

# Node 22 from the official image (node:22.23.3-trixie-slim): the program and npm, nothing else.
COPY --from=node:22.23.3-trixie-slim@sha256:28d03803f09ba76560e273af33beb7eda4ed68fa0698bc9bd15e9a6e35693923 /usr/local/bin/node /usr/local/bin/node
COPY --from=node:22.23.3-trixie-slim@sha256:28d03803f09ba76560e273af33beb7eda4ed68fa0698bc9bd15e9a6e35693923 /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx && node --version && npm --version

# The WebAssembly target and wasm-pack, at one version (checked against crates.io's checksums by --locked). wasm-pack fetches the
# wasm-bindgen program that matches Cargo.lock the first time it builds.
RUN rustup target add wasm32-unknown-unknown
RUN --mount=type=cache,target=/usr/local/cargo/registry cargo install wasm-pack --locked --version 0.15.0

WORKDIR /src
# npm packages first: they change less often than the code, so this layer is kept.
COPY package.json package-lock.json ./
COPY packages/core/package.json packages/core/
COPY packages/extensions/package.json packages/extensions/
COPY packages/editor/package.json packages/editor/
COPY web/package.json web/
RUN --mount=type=cache,target=/root/.npm npm ci

COPY . .
# The build, with cargo's downloads and compiled crates kept between builds (cache mounts: not part of the image).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    --mount=type=cache,target=/root/.cache/.wasm-pack \
    node scripts/build-web.mjs --no-reuse

# Each compressible file is compressed once here (nginx's gzip_static sends the .gz to a browser that accepts it).
RUN cd web/dist && find . -type f -size +1k \( -name '*.wasm' -o -name '*.js' -o -name '*.css' -o -name '*.html' -o -name '*.svg' -o -name '*.json' -o -name '*.webmanifest' -o -name '*.txt' -o -name '*.aff' -o -name '*.dic' \) -exec gzip -9 -k -n {} +

# --- run: nginx as a user that is not root, port 8080 ----------------------------------------------------------------------------------
# nginxinc/nginx-unprivileged:1.31.7-alpine (the mainline line, which nginx recommends for production; runs as user 101 and keeps its pid
# file and temporary files in /tmp). Not the stable line: its newest unprivileged image (1.30.6) carried 4 critical
# and 2 high nginx problems that 1.30.7 fixes, and no unprivileged 1.30.7 image existed. Trivy finds no nginx problem in 1.31.7.
FROM nginxinc/nginx-unprivileged:1.31.7-alpine@sha256:b5047b9b76845eac124358373c6852377c52e4b05834a47647e955dd91cb8f1e

# The base image is a snapshot: take the fixes Alpine has published for its packages since (zlib, libexpat, ...) so the scan in the release
# workflow starts clean. nginx itself comes from nginx.org, so an nginx fix arrives with a newer base image: move the digest above.
USER root
RUN apk upgrade --no-cache
USER 101

ARG VERSION=dev
ARG REVISION=unknown
LABEL org.opencontainers.image.title="docxtor-web" \
      org.opencontainers.image.description="The Docxtor web app, a Word-compatible .docx editor that runs in the browser, served by nginx." \
      org.opencontainers.image.source="https://github.com/tinycommit-dev/docxtor" \
      org.opencontainers.image.url="https://docxtor.tinycommit.dev" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}"

COPY --from=build /src/web/dist /usr/share/nginx/html
COPY deploy/nginx.conf /etc/nginx/docxtor-headers.conf
COPY deploy/docker/default.conf /etc/nginx/conf.d/default.conf
# The licence notices travel with the image: licenses.txt is also served at /licenses.txt.
COPY LICENSE-APACHE LICENSE-MIT THIRD_PARTY_NOTICES.md /usr/share/doc/docxtor/
COPY --from=build /src/web/dist/licenses.txt /usr/share/doc/docxtor/licenses.txt

EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1
