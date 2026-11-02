# Registry + reserve pages. Chain data, fund profiles and attestations live in /data.
#   docker build -t zsolv .
#   docker run -p 8080:8080 -v zsolv-data:/data zsolv
FROM rust:1.98-slim-bookworm AS rust
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --locked --bin zsolv && strip target/release/zsolv

FROM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web ./
RUN npm run build

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 zsolv && mkdir /data && chown zsolv /data
COPY --from=rust /src/target/release/zsolv /usr/local/bin/zsolv
COPY --from=web /web/dist /srv/web
USER zsolv
ENV ZSOLV_DATA=/data ZSOLV_ADDR=0.0.0.0:8080
VOLUME /data
EXPOSE 8080
CMD ["zsolv", "serve", "--web", "/srv/web"]
