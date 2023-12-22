# First, we create a build plan to share between intermediate build steps
FROM lukemathwalker/cargo-chef as planner
WORKDIR /app
COPY ./twamp ./
RUN cargo chef prepare --recipe-path recipe.json

# Next, we cache the dependencies
FROM lukemathwalker/cargo-chef as cacher
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json

# Finally, we build the actual application
FROM rust:latest as builder
WORKDIR /app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN cargo build --release --example twamp_example

# Use a smaller image for deployment
FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/examples/twamp_example /usr/local/bin/twamp
COPY --from=builder /app/twamp/log_config.yml /app/log_config.yml
CMD ["twamp", "/app/twamp_config.json"]
