#!/bin/bash

# Initialize variables
DOCKERFILE=Dockerfile
REGISTRY=
IMAGE_NAME=
TAG=latest
PLATFORMS="linux/amd64,linux/arm64"

# Define usage function
function usage {
  echo "Usage: $0 -f <dockerfile> -i <image_name> [-r <registry>] [-t <tag>] [-p <platforms>]"
  echo "Example: $0 -f Dockerfile -i my-image -r my-registry -t latest -p linux/amd64,linux/arm64"
  exit 1
}

# Parse options
while getopts "f:i:r:t:p:" opt; do
  case ${opt} in
    f ) DOCKERFILE=$OPTARG ;;
    i ) IMAGE_NAME=$OPTARG ;;
    r ) REGISTRY=$OPTARG ;;
    t ) TAG=$OPTARG ;;
    p ) PLATFORMS=$OPTARG ;;
    * ) usage ;;
  esac
done

# Check required options
if [[ -z "$IMAGE_NAME" ]]; then
  usage
fi

# Construct composed name
if [[ -n "$REGISTRY" ]]; then
  COMPOSED_NAME="$REGISTRY/$IMAGE_NAME:$TAG"
else
  COMPOSED_NAME="$IMAGE_NAME:$TAG"
fi

# Build and push image
if [[ -n "$REGISTRY" ]]; then
  echo "Pushing Docker image '$COMPOSED_NAME' to registry..."
  docker buildx build --platform $PLATFORMS --push -t $COMPOSED_NAME -f $DOCKERFILE .
else 
  echo "Building Docker image '$COMPOSED_NAME' using Dockerfile '$DOCKERFILE' for platforms: $PLATFORMS ..."
  docker buildx build --platform $PLATFORMS -t $COMPOSED_NAME -f $DOCKERFILE .
fi
