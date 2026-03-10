#!/bin/bash 
set -x 
set -eo pipefail

DOCKER_CONTAINER=$(docker ps --filter publish=6379 --format '{{.ID}}')
if [[ -n $DOCKER_CONTAINER ]]; then
  echo >&2 "Redis ports already in use in another container; Stop $DOCKER_CONTAINER before continuing."
  exit 1
fi

if docker start redis_zero2prod >/dev/null 2>&1; then
  echo "Redis is running on port 6379"
else 
  docker run --rm -d --name redis_zero2prod -p 6379:6379 redis:latest
  echo "Redis is running on port 6379"
fi 

exit 0
