#!/bin/bash

# Initialize variables
DOCKERFILE=Dockerfile
REGISTRY=
IMAGE_NAME=
TAG=latest

# Define usage function
function usage {
  echo "Usage: $0 -f <dockerfile> -i <image_name> [-r <registry>] [-t <tag>]"
  echo "Example: $0 -f Dockerfile -i my-image -r my-registry -t latest"
  exit 1
}

# Parse options
while getopts "f:i:r:t:" opt; do
  case ${opt} in
    f ) DOCKERFILE=$OPTARG ;;
    i ) IMAGE_NAME=$OPTARG ;;
    r ) REGISTRY=$OPTARG ;;
    t ) TAG=$OPTARG ;;
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
echo "Building Docker image '$COMPOSED_NAME' using Dockerfile '$DOCKERFILE'..."
docker build -t $COMPOSED_NAME -f $DOCKERFILE .

if [[ -n "$REGISTRY" ]]; then
  echo "Pushing Docker image '$COMPOSED_NAME' to registry..."
  docker push $COMPOSED_NAME
fi
