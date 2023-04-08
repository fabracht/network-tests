FROM ubuntu:latest
WORKDIR /app

RUN apt update && apt install -y iproute2
COPY ./twamp/log_config.yml /app/
COPY ./twamp/sender_config.json /app/
COPY ./target/release/twamp /app/

CMD ["/app/twamp", "-c", "/app/sender_config.json"]