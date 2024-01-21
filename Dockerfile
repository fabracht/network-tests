# First, we create a build plan to share between intermediate build steps
FROM rust:latest as planner
WORKDIR /app
RUN cargo install cargo-chef
COPY ./twamp ./
RUN cargo chef prepare --recipe-path recipe.json

# Next, we cache the dependencies
FROM rust:latest as cacher
WORKDIR /app
RUN cargo install cargo-chef
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
FROM debian:latest
WORKDIR /app
COPY --from=builder /app/target/release/examples/twamp_example /usr/local/bin/twamp
COPY --from=builder /app/twamp/log_config.yml /usr/local/bin/log_config.yml
# COPY --from=builder /app/twamp/examples/configurations/receiver_config.json /usr/local/bin/receiver_config.json
# CMD ["/usr/local/bin/twamp", "/usr/local/bin/receiver_config.json"]
ENTRYPOINT ["/usr/local/bin/twamp"]
