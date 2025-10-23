# PostgreSQL Testing Setup

## Quick Start

1. **Setup credentials:**
   ```bash
   cp .env.postgres.example .env.postgres
   # Edit .env.postgres with secure database credentials
   ```

2. **Run all tests:**
   ```bash
   ./run-local-tests.sh
   ```

## What `run-local-tests.sh` Does

- **Docker Environment**: Starts PostgreSQL 15 container using docker-compose
- **Database Setup**: Creates fresh database with your credentials from `.env.postgres`
- **Health Check**: Waits for PostgreSQL to be ready (up to 30 seconds)
- **Test Execution**: Runs complete test suite (SQLite + PostgreSQL tests)
- **Cleanup**: Stops and removes PostgreSQL container after completion

## Dependencies

- **Docker**: Required for PostgreSQL container
- **Credential File**: `.env.postgres` with database password
- **Environment Variables**: Auto-configured for test execution

## GitHub CI/CD

- **Secrets**: `POSTGRES_PASSWORD` required in repository settings
- **Auto-triggers**: PRs and pushes to main/develop
- **Isolation**: Ephemeral databases, no data persistence
- **Workflow**: `.github/workflows/postgresql.yml`

## Files

| File | Purpose |
|------|---------|
| `run-local-tests.sh` | Automated test runner |
| `.env.postgres.example` | Credential template |
| `.env.postgres` | Your credentials (gitignored) |
| `docker-compose.yml` | PostgreSQL container config |
| `tests/postgresql_env_setup.rs` | PostgreSQL connectivity tests |
