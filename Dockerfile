# Use an official Rust runtime as a parent image
FROM rust:latest as build
# Create a new empty shell project
WORKDIR /

# Copy the Cargo.toml and Cargo.lock files and build dependencies
COPY ./common ./common
COPY ./message_macro ./message_macro
COPY ./twamp ./twamp
COPY ./Cargo.lock ./Cargo.toml ./
RUN cat Cargo.toml
RUN cargo build --release --bin twamp

# Use a smaller image for deployment
FROM debian:latest
COPY --from=build /target/release/twamp /usr/local/bin/twamp
COPY --from=build /twamp/log_config.yml /usr/local/bin/log_config.yml
COPY --from=build /twamp/configurations/receiver_config.json /usr/local/bin/receiver_config.json
RUN ls /usr/local/bin
CMD ["/usr/local/bin/twamp", "-c", "/usr/local/bin/receiver_config.json"]
# CMD sleep 1000000
# RUN apk add --no-cache openssl && \
#     rm -rf /var/cache/apk/*