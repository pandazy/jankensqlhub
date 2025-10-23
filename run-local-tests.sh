#!/bin/bash

# JankenSQLHub Local Test Runner
# This script sets up a fresh PostgreSQL container, runs all tests, and cleans up

set -e

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Error: Docker is not installed or not available in PATH"
    echo "Please install Docker Desktop or Docker Engine:"
    echo "  - macOS: https://docs.docker.com/desktop/install/mac-install/"
    echo "  - Linux: https://docs.docker.com/engine/install/"
    echo "  - Windows: https://docs.docker.com/desktop/install/windows-install/"
    exit 1
fi

# Check if Docker daemon is running
if ! docker info &> /dev/null; then
    echo "❌ Error: Docker daemon is not running"
    echo "Please start Docker Desktop or Docker service and try again"
    exit 1
fi

# Load environment variables from .env.postgres
if [ -f .env.postgres ]; then
    export $(grep -v '^#' .env.postgres | xargs)
    # Set PostgreSQL connection string for tests
    export POSTGRES_CONNECTION_STRING="host=localhost user=${POSTGRES_USER:-jankensqlhub_user} password=${POSTGRES_PASSWORD} dbname=${POSTGRES_DB:-jankensqlhub_test}"
fi

echo "🏗️  Setting up PostgreSQL environment..."

# Start PostgreSQL container
docker compose up -d

# Wait for PostgreSQL to be ready
echo "⏳ Waiting for PostgreSQL to be ready..."
max_attempts=30
attempt=1

while [ $attempt -le $max_attempts ]; do
    if docker compose exec postgres pg_isready -U "${POSTGRES_USER:-jankensqlhub_user}" > /dev/null 2>&1; then
        echo "✅ PostgreSQL is ready!"
        break
    fi

    if [ $attempt -eq $max_attempts ]; then
        echo "❌ PostgreSQL failed to start within expected time"
        docker compose down
        exit 1
    fi

    echo "Attempt $attempt/$max_attempts: PostgreSQL not ready yet..."
    sleep 2
    ((attempt++))
done

# Run all tests
echo "🧪 Running all tests..."
cargo test

echo "✅ All tests passed!"

# Clean up
echo "🧹 Cleaning up PostgreSQL environment..."
docker compose down

echo "✨ Test run complete!"
