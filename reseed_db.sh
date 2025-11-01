#!/usr/bin/env bash

set -e

echo "* Installing refinery_cli..."
cargo install refinery_cli


if [ -f .env ]; then
  echo "* Loading environment variables from .env file"
  set -a
  . ./.env
  set +a
fi

if [ -z "$DATABASE_URL" ]; then
  echo "Error: DATABASE_URL is not set."
  exit 1
fi

echo "* Dropping all tables in the database..."
psql "$DATABASE_URL" -t -A -c "
  SELECT 'DROP TABLE IF EXISTS ' || quote_ident(schemaname) || '.' || quote_ident(tablename) || ' CASCADE;'
  FROM pg_tables
  WHERE schemaname = 'public';
" | psql "$DATABASE_URL"

echo "* Running migrations..."
~/.cargo/bin/refinery migrate -p ./migrations -e DATABASE_URL


echo "* Seeding the database..."
psql "$DATABASE_URL" -f ./seeds/seed.sql


echo "* Importing 'the hub' blueprint..."
cargo run --bin import-yaml -- --bp-key hub --owner system --subdir rooms --entry-room cell_block
cargo run --bin create-realm -- --bp-key hub --title "Live World" --key "live_world" --owner system --kind live

