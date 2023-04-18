FROM alpine:latest

# Install Rust and Cargo
RUN apk update && \
    apk add --no-cache rust cargo

# Create and set the working directory
WORKDIR /app

# Copy the project files to the working directory
COPY . .

# Set the command to run the application
CMD ["cargo", "run", "--release", "--bin", "twamp", "--", "-c", "/app/twamp/configuration/config.json"]

# Mount the configuration folder
VOLUME /app/twamp/configuration
