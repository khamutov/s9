# Stage 1: Frontend build
FROM node:22-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

# Stage 2: Backend build
FROM rust:1.85-bookworm AS backend
WORKDIR /app

# Cache Rust dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src target/release/s9*

# Copy frontend build output
COPY --from=frontend /app/frontend/dist frontend/dist/

# Copy real source and build
COPY src/ src/
COPY migrations/ migrations/
RUN cargo build --release

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -r -s /bin/false s9
COPY --from=backend /app/target/release/s9 /usr/local/bin/s9

USER s9
EXPOSE 8080
VOLUME /data
ENV S9_DATA_DIR=/data
ENV S9_LISTEN=0.0.0.0:8080

ENTRYPOINT ["s9"]
CMD ["serve"]
