# Shift Scheduling System

A shift scheduling system composed of two Rust microservices -- a **Data Service** for staff/group management and a **Scheduling Service** for asynchronous shift schedule generation.

## Architecture

```
shift-scheduler/
- data-service/          # Staff, groups, memberships (PostgreSQL + Redis)
- scheduling-service/    # Async schedule generation (PostgreSQL)
- shared/                # Common types, responses, telemetry, shutdown
- sample-data/           # JSON files for batch import
- docker-compose.yml     # Full stack: postgres, redis, jaeger, both services
```

Both services have three layers:

- **API** -- Axum handlers, request/response types, app state
- **Domain** -- Traits (ports), entities, business logic
- **Infrastructure** -- PostgreSQL repositories, Redis cache, HTTP client

## Tech Stack

| Component        | Choice                                             |
| ---------------- | -------------------------------------------------- |
| Language         | Rust (edition 2024)                                |
| Web Framework    | Axum 0.8                                           |
| Async Runtime    | Tokio                                              |
| Database         | PostgreSQL 17 (sqlx, compile-time checked queries) |
| Caching          | Redis 8                                            |
| Serialization    | serde / serde_json                                 |
| API Docs         | utoipa (OpenAPI 3.0 / Swagger UI)                  |
| Tracing          | OpenTelemetry + Jaeger                             |
| Containerization | Docker, docker-compose                             |

## Quick Start

### Prerequisites

- Docker and docker-compose

### Run

```bash
docker-compose up --build
```

This starts all services:

| Service              | URL                              |
| -------------------- | -------------------------------- |
| Data Service API     | http://localhost:8180            |
| Data Service Swagger | http://localhost:8180/swagger-ui |
| Scheduling Service   | http://localhost:8181            |
| Scheduling Swagger   | http://localhost:8181/swagger-ui |
| Jaeger UI            | http://localhost:16686           |

## Sample Data Import

### Automatic (via Docker Compose)

When you run `docker-compose up`, a one-shot `seed` container automatically imports
staff and groups from the `sample-data/` directory into the data-service.

- **Staff** and **groups** are imported on first run
- Re-running is safe -the seed script detects existing data and skips
- **Memberships** require real UUIDs, so they must be created manually via the API

### Manual

1. `POST /api/v1/staff/batch` with `staff.json`
2. `POST /api/v1/groups/batch` with `groups.json`
3. `PUT /api/v1/groups/{id}` to set `parent_group_id` using returned IDs
4. `POST /api/v1/memberships/batch` with real staff/group UUIDs

## Database Schema

### Data Service (`data_service_db`)

**staff** -- id (uuid PK), name, email (unique), position, status (ACTIVE/INACTIVE),
created_at, updated_at

**staff_groups** -- id (uuid PK), name, parent_group_id (FK self, ON DELETE SET
NULL), created_at, updated_at

**group_memberships** -- staff_id (FK staff CASCADE), group_id (FK staff_groups
CASCADE), composite PK

### Scheduling Service (`scheduling_service_db`)

**schedule_jobs** -- id (uuid PK), staff_group_id, period_begin_date, status
(PENDING/PROCESSING/COMPLETED/FAILED), created_at, updated_at

**shift_assignments** -- id (uuid PK), job_id (FK schedule_jobs CASCADE), staff_id,
date, shift_type (MORNING/EVENING/DAY_OFF)

## API Overview

### Data Service (port 8180)

#### Staff

| Method | Path                          | Description        |
| ------ | ----------------------------- | ------------------ |
| GET    | /api/v1/staff                 | List all staff     |
| GET    | /api/v1/staff/{id}            | Get staff by ID    |
| POST   | /api/v1/staff                 | Create staff       |
| POST   | /api/v1/staff/batch           | Batch create staff |
| PUT    | /api/v1/staff/{id}            | Update staff       |
| PATCH  | /api/v1/staff/{id}/deactivate | Deactivate staff   |
| DELETE | /api/v1/staff/{id}            | Delete staff       |

#### Groups

| Method | Path                 | Description         |
| ------ | -------------------- | ------------------- |
| GET    | /api/v1/groups       | List all groups     |
| GET    | /api/v1/groups/{id}  | Get group by ID     |
| POST   | /api/v1/groups       | Create group        |
| POST   | /api/v1/groups/batch | Batch create groups |
| PUT    | /api/v1/groups/{id}  | Update group        |
| DELETE | /api/v1/groups/{id}  | Delete group        |

#### Memberships

| Method | Path                                         | Description                              |
| ------ | -------------------------------------------- | ---------------------------------------- |
| POST   | /api/v1/groups/{group_id}/members            | Add staff to group                       |
| POST   | /api/v1/memberships/batch                    | Batch add members                        |
| DELETE | /api/v1/groups/{group_id}/members/{staff_id} | Remove staff from group                  |
| GET    | /api/v1/groups/{group_id}/members            | List direct members                      |
| GET    | /api/v1/groups/{group_id}/resolved-members   | List members incl. subgroups (recursive) |
| GET    | /api/v1/staff/{id}/groups                    | List staff's groups                      |

### Scheduling Service (port 8181)

| Method | Path                                   | Description               |
| ------ | -------------------------------------- | ------------------------- |
| POST   | /api/v1/schedules                      | Submit schedule job (202) |
| GET    | /api/v1/schedules/{schedule_id}/status | Check job status          |
| GET    | /api/v1/schedules/{schedule_id}/result | Get generated schedule    |

Full interactive API documentation is available at each service's `/swagger-ui` endpoint.

## Scheduling Rules

The scheduler generates 28-day (4-week) schedules with these configurable constraints:

| Rule                      | Key                      | Default |
| ------------------------- | ------------------------ | ------- |
| Min days off per week     | min_day_off_per_week     | 1       |
| Max days off per week     | max_day_off_per_week     | 2       |
| No MORNING after EVENING  | no_morning_after_evening | true    |
| Max daily shift imbalance | max_daily_shift_diff     | 1       |

## Caching

Read-heavy data-service endpoints are cached in Redis with automatic invalidation on mutations:

- Staff queries: 5-10 min TTL
- Group queries: 5-10 min TTL
- Membership/resolved-member queries: 5 min TTL

Write operations invalidate related cache entries (including cross-entity invalidation for membership changes).

## Observability

- **Structured logging** via `tracing` with configurable format (JSON/text via `LOG_FORMAT` env var)
- **Distributed tracing** via OpenTelemetry with OTLP export to Jaeger
- **Trace propagation** between services (scheduling-service injects trace context into HTTP calls to data-service)
- **Jaeger UI** at http://localhost:16686 for viewing request traces across services

## Testing

```bash
# Run all tests
cargo test --workspace

# Run with test-support features (for mockall)
cargo test --workspace --features test-support

# Linting
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

## Design Decisions

- **Type-state pattern** for job lifecycle -- compile-time guarantee that only valid state transitions occur (Pending -> Processing -> Completed/Failed)
- **Decorator pattern** for caching -- CachedRepository wraps PgRepository, same trait interface
- **Recursive CTE** for resolved-members -- single SQL query resolves entire group hierarchy
- **TaskTracker** for async jobs -- graceful shutdown waits for in-flight background jobs (30s timeout)
- **Compile-time SQL** -- sqlx macros verify queries against the database schema at build time

## Note:

### Performance Consideration

- HTTP client responses use owned deserialization (`String` / `DeserializeOwned`).
  For high-throughput scenarios involving large payloads, zero-copy deserialization
  (`&str` / `Deserialize<'de>`) could reduce heap allocations by borrowing directly
  from the response buffer. This was not implemented as the current data volume
  (tens of staff per request) does not warrant the added lifetime complexity.

### Algorithm & Known Limitations

The scheduler uses a greedy assignment algorithm: for each day, it iterates through
staff members and assigns the first valid shift that satisfies all enabled rules. If a case, 3 staff taking EVENING the pervious day, then the config `no_morning_after_evening=true`, Morning is block for everyone. Remaining staff must take DAY_OFF, but if they don't have day off left, which is a disaster as well. I'm thinking if an algo could resolve this.

## Future Work/On-Planning

- **Circuit breaker** for Data Service calls -- would prevent cascade failures when data-service is unavailable by failing fast and auto-recovering after a configurable timeout. The decorator pattern (same approach as `CachedRepository`) makes this straightforward to add.
- Just incase, I made some improvement on **improvement** branch, since I'm out of time on the submit deadline, I will merge later.
