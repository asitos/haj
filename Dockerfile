# builder
FROM archlinux:base-devel AS builder

RUN pacman -Syu --noconfirm rust cargo clang pkgconf

WORKDIR /usr/src/haj

COPY . .

RUN cargo install display3d --version 0.2.3 --locked \
 && cargo build --release

# runtime
FROM archlinux:base

RUN pacman -Syu --noconfirm pacman sudo ca-certificates \
 && pacman -Scc --noconfirm

WORKDIR /app

COPY --from=builder /usr/src/haj/target/release/haj /usr/local/bin/haj
COPY --from=builder /root/.cargo/bin/display3d /usr/local/bin/display3d

ENTRYPOINT ["haj"]
CMD ["--help"]
