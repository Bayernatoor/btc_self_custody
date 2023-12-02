#!/usr/bin/env bash

# enable debugging - prints each line
set -x 
# exit immediately if any command exits with non 0 error
# if any command in a pipeline fails the entire pipeline returns a failure status
set -eo pipefail

# ensure psql is installed
if ! [ -x "$(command -v psql)" ]; then
    echo >&2 "Error: `psql` is not installed."
    exit 1
fi

# if sqlx not installed install it
if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: `sqlx` is not installed."
  echo >&2 "Use:"
  echo >&2 "    cargo install --version=0.5.7 sqlx-cli --no-default-features --features postgres"
  echo >&2 "to install it."
  exit 1
fi

#check if a custom user has been set, otherwise default to "postgres"
DB_USER=${POSTGRES_USER:=postgres}
# check if a custom password has been set, otherwise default to "password"
DB_PASSWORD="${POSTGRES_PASSWORD:=password}"
# Check if a custom database name has been set, otherwise default to "newsletter"
DB_NAME="${POSTGRES_DB:=blog}"
# Check if a custom port has been set, otherwise default to "5432"
DB_PORT="${POSTGRES_PORT:=5432}"

# Allow to skip docker if a dockerized postgres db is already running
if [[ -z "${SKIP_DOCKER}" ]]
then
# Launch Postgres using Docker
docker run \
    -e POSTGRES_USER=${DB_USER} \
    -e POSTGRES_PASSWORD=${DB_PASSWORD} \
    -e POSTGRES_DB=${DB_NAME} \
    -p "${DB_PORT}":5432 \
    -d postgres \
    postgres -N 1000
fi

# keep pinging Postgres until it's ready to accept commands
export PGPASSWORD="${DB_PASSWORD}"
until psql -h "localhost" -U "${DB_USER}" -p "${DB_PORT}" -d "postgres" -c '\q'; do
  >&2 echo "Postgres is still unavailable - sleeping"
  sleep 1
done

>&2 echo "Postgres is up and running on port ${DB_PORT}!"

export DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}
sqlx database create
sqlx migrate run

>&2 echo "Postgres has been migrated, ready to go!"
