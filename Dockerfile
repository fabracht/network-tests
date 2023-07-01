# First, we create a build plan to share between intermediate build steps
FROM rust:latest as planner
WORKDIR /app
RUN cargo install cargo-chef
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./common ./common
COPY ./message_macro ./message_macro
COPY ./twamp ./twamp
RUN cargo chef prepare  --recipe-path recipe.json

# Next, we cache the dependencies
FROM rust:latest as cacher
WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Finally, we build the actual application
FROM rust:latest as builder
WORKDIR /app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN cargo build --release --bin twamp

# Use a smaller image for deployment
FROM debian:latest
WORKDIR /app
COPY --from=builder /app/target/release/twamp /usr/local/bin/twamp
COPY --from=builder /app/twamp/log_config.yml /usr/local/bin/log_config.yml
COPY --from=builder /app/twamp/configurations/receiver_config.json /usr/local/bin/receiver_config.json
RUN ls /usr/local/bin
CMD ["/usr/local/bin/twamp", "-c", "/usr/local/bin/receiver_config.json"]
