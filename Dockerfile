# builder
FROM archlinux:base-devel AS builder

RUN pacman -Syu --noconfirm rust cargo clang pkgconf

WORKDIR /usr/src/haj

COPY . .

RUN cargo build --release

# runtime
FROM archlinux:base

RUN pacman -Syu --noconfirm && pacman -Scc --noconfirm

WORKDIR /app

COPY --from=builder /usr/src/haj/target/release/haj /usr/local/bin/haj

# default is help
CMD ["haj", "--help"]
