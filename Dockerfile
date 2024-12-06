# ------------------------------------------------------------------------------
# Cargo Build Stage
# ------------------------------------------------------------------------------

FROM rust:latest AS prepare-build
RUN apt-get update
RUN apt-get install pkg-config libssl-dev -y
RUN cargo install cargo-chef
WORKDIR /app

FROM prepare-build AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM prepare-build AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM archlinux AS coordinator
WORKDIR /home/coordinator/bin/
COPY --from=builder /app/target/release/coordinator .
CMD ["./coordinator"]

FROM archlinux:multilib-devel AS worker
RUN groupadd -g 1000 worker
RUN useradd -s /bin/sh -u 1000 -g worker worker
RUN echo 'worker ALL=(ALL:ALL) NOPASSWD: ALL' > /etc/sudoers.d/worker
RUN echo 'OPTIONS=(!strip docs libtool staticlibs emptydirs !zipman !purge !debug !lto !autodeps)' > /etc/makepkg.conf.d/options.conf
WORKDIR /home/worker/bin/
RUN chown -R worker:worker /home/worker
USER worker
RUN sudo pacman -Sy --needed --noconfirm base-devel git
RUN git clone https://aur.archlinux.org/paru-bin.git
RUN makepkg -D paru-bin --noconfirm -si
RUN rm -rf paru
COPY --from=builder /app/target/release/worker .
CMD ["./worker"]