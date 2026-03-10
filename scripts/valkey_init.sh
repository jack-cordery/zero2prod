#!/bin/bash 
set -x 
set -eo pipefail

DOCKER_CONTAINER=$(docker ps --filter publish=6379 --format '{{.ID}}')
if [[ -n $DOCKER_CONTAINER ]]; then
  echo >&2 "Valkey ports already in use in another container; Stop $DOCKER_CONTAINER before continuing."
  exit 1
fi

if ! docker start valkey_zero2prod >/dev/null 2>&1; then
  docker run --rm -d --name valkey_zero2prod -p 6379:6379 valkey/valkey:latest
fi 

echo "Valkey is running on port 6379"
exit 0
