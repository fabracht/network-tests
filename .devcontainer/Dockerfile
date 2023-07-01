# Choose your desired base image
FROM rust:latest

# The USER_UID and USER_GID can be set at build time to match your user id on your host machine.
# This can help avoid issues with file permissions for shared volumes.
# By default, they are set to 1000, which is the first "regular" user on a linux machine.
ARG USER_UID=1000
ARG USER_GID=$USER_UID

# The username for the new user
ARG USERNAME=vscode
# Install sudo
RUN apt-get update && \
    apt-get install -y sudo

# Create the user with the specified USER_UID and add it to sudoers
RUN groupadd --gid $USER_GID $USERNAME \
    && useradd --uid $USER_UID --gid $USERNAME --shell /bin/bash --create-home $USERNAME \
    && mkdir -p /etc/sudoers.d \
    && echo $USERNAME ALL=\(root\) NOPASSWD:ALL > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME

# Switch to the new user
USER $USERNAME

# Change the working directory to the new user's home directory
WORKDIR /home/$USERNAME

# Install Rust components, if any
RUN rustup component add rustfmt
