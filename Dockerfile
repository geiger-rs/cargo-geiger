FROM rust as chef
RUN cargo install cargo-chef --locked
WORKDIR app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --offline --locked --frozen --bin cargo-geiger

FROM rust:slim as runtime
RUN apt-get update \
    && apt-get install --no-install-recommends -y libcurl4 \
    && apt-get clean
WORKDIR "/workdir"
COPY --from=builder /app/target/release/cargo-geiger /usr/local/bin/cargo-geiger
ENTRYPOINT ["cargo-geiger"]
