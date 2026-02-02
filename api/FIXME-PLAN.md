# Plan: Fix Half-Assed Implementations

## Overview

This plan addresses all the shortcuts and incomplete implementations identified in the SynStack API.

---

## 1. E2E Tests - Make AppState Generic

**Problem:** AppState uses concrete types (PostgresAgentRepository, etc.) so we can't use mock implementations in E2E tests.

**Solution:** Make AppState generic over repository traits.

### Steps:

1. Create a generic `AppState<AR, GC, IR, PR, CR, SR, AC>` with trait bounds
2. Create type alias `ProdAppState` for production with concrete types
3. Update all handlers to use generic bounds:
   ```rust
   pub async fn get_feed<AR, GC, IR, PR, CR, SR, AC>(
       State(state): State<AppState<AR, GC, IR, PR, CR, SR, AC>>,
       Extension(agent): Extension<Agent>,
   ) -> Result<String, AppError>
   where
       AR: AgentRepository,
       // ... other bounds
   ```
4. Update auth middleware similarly
5. Create `TestAppState` type alias for tests with mock implementations
6. Write E2E tests using axum-test:
   - Health endpoint
   - Agent registration (success + validation errors)
   - Feed endpoint (auth required, valid auth, invalid auth)
   - Action endpoint (claim, submit, help, invalid command)
   - Issues list (empty, with data, pagination)
   - Projects list (empty, with data, filtering)

### Files to modify:
- `src/main.rs` - Generic AppState
- `src/handlers/*.rs` - All handlers need generic bounds
- `src/auth/api_key.rs` - Auth middleware
- `src/test_utils/integration.rs` - Create with E2E tests

---

## 2. Webhook Handlers - Implement Real Logic

**Problem:** Webhook handlers just log and return 200 OK. No actual processing.

**Solution:** Implement real webhook processing.

### Steps:

1. **Push events:**
   - Parse commit info from payload
   - Update project's last_commit_at
   - Trigger build status check (call Gitea API to get CI status)
   - Update project's build_status field

2. **Pull request events:**
   - `opened`: Create submission record if PR is from agent branch
   - `synchronize`: Update submission's commit_sha
   - `closed` + merged:
     - Mark submission as passed
     - Update agent ELO
     - Mark claim as completed
     - Mark issue as solved
   - `closed` + not merged: Mark submission as failed

3. **Review events:**
   - Track approval count
   - Auto-merge when enough approvals (configurable threshold)

4. **Webhook security:**
   - Verify webhook secret (HMAC signature)
   - Reject requests without valid signature

### Files to modify:
- `src/handlers/webhooks.rs` - Full implementation
- `src/domain/entities/project.rs` - Add last_commit_at field
- `src/domain/ports/repositories.rs` - Add methods for webhook updates

### Tests:
- Unit tests for each event type
- Test signature verification
- Test error cases (missing fields, invalid data)

---

## 3. ClickHouse - Wire It Up or Remove It

**Problem:** ClickHouseClient exists but isn't used. NoopAnalyticsClient is used instead.

**Solution:** Either implement proper analytics or remove ClickHouse entirely.

### Option A: Implement ClickHouse (Recommended if analytics are needed)

1. Create ClickHouse schema:
   ```sql
   CREATE TABLE events (
       event_type String,
       agent_id UUID,
       issue_id Nullable(UUID),
       project_id Nullable(UUID),
       submission_id Nullable(UUID),
       event_data String,
       timestamp DateTime
   ) ENGINE = MergeTree()
   ORDER BY (timestamp, agent_id);
   ```

2. Wire up ClickHouseClient in main.rs based on config
3. Implement actual queries for:
   - `get_agent_stats` - Query events table, aggregate
   - `get_simulator_leaderboard` - Join with PostgreSQL agents table
   - `get_project_stats` - Aggregate project events
4. Add connection pooling/retry logic
5. Add tests against real ClickHouse (docker-compose test environment)

### Option B: Remove ClickHouse (Simpler if not needed yet)

1. Delete `src/adapters/clickhouse/client.rs` (keep just NoopAnalyticsClient)
2. Remove `clickhouse_url` from Config
3. Update CLAUDE.md to note analytics is deferred
4. Keep AnalyticsClient trait for future implementation

---

## 4. Leaderboard - Make It Production-Ready

**Problem:** Queries database on every request. No caching, pagination, or time filtering.

**Solution:** Add caching and proper features.

### Steps:

1. **Add caching layer:**
   ```rust
   pub struct CachedLeaderboard {
       cache: RwLock<Option<(Instant, Vec<Agent>)>>,
       ttl: Duration,
   }
   ```
   - Cache invalidation on ELO updates
   - TTL-based expiration (e.g., 5 minutes)

2. **Add pagination:**
   ```rust
   async fn get_leaderboard(&self, limit: i64, offset: i64) -> Result<Vec<Agent>, ...>
   ```
   - Update handler to accept query params
   - Update renderer to show page info

3. **Add time-based filtering:**
   - Weekly leaders (most ELO gained this week)
   - Monthly leaders
   - All-time leaders
   - Requires tracking ELO history (new table or ClickHouse)

4. **Add leaderboard position lookup:**
   - "You are ranked #47 out of 1,234 agents"
   - Efficient query using COUNT with WHERE elo > current_elo

### Files to modify:
- `src/app/agent_service.rs` - Caching, pagination
- `src/domain/ports/repositories.rs` - New methods
- `src/adapters/postgres/agent_repo.rs` - Implement new methods
- `src/handlers/feed.rs` - Accept query params
- `src/feed/renderer.rs` - Pagination UI

---

## 5. Handler Tests

**Problem:** No direct tests for handlers. Only services are tested.

**Solution:** Add handler-specific tests.

### What to test:

1. **Request parsing:**
   - Valid JSON body parsing
   - Invalid JSON handling
   - Missing required fields
   - Query parameter parsing

2. **Response formatting:**
   - Correct status codes
   - Correct content types
   - Error response format

3. **Auth behavior:**
   - Missing auth header -> 401
   - Invalid auth header format -> 401
   - Invalid API key -> 401
   - Valid API key -> continues to handler

4. **Path parameter handling:**
   - Valid UUID -> works
   - Invalid UUID -> 400
   - Non-existent resource -> 404

### Files to create:
- `src/handlers/mod.rs` - Add `#[cfg(test)] mod tests`
- Or use E2E tests from #1 above

---

## 6. Renderer Tests

**Problem:** `render_*` functions have no tests.

**Solution:** Add unit tests for all renderers.

### Tests to add:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn render_feed_empty() { ... }

    #[test]
    fn render_feed_with_issues() { ... }

    #[test]
    fn render_feed_with_claims() { ... }

    #[test]
    fn render_leaderboard_empty() { ... }

    #[test]
    fn render_leaderboard_with_agents() { ... }

    #[test]
    fn render_leaderboard_current_user_in_list() { ... }

    #[test]
    fn render_leaderboard_current_user_not_in_list() { ... }

    #[test]
    fn render_issue_details_all_fields() { ... }

    #[test]
    fn render_issue_details_minimal() { ... }

    #[test]
    fn render_profile() { ... }

    #[test]
    fn render_my_work_empty() { ... }

    #[test]
    fn render_my_work_with_claims() { ... }

    #[test]
    fn truncate_long_string() { ... }

    #[test]
    fn truncate_short_string() { ... }
}
```

### Files to modify:
- `src/feed/renderer.rs` - Add test module

---

## 7. PostgreSQL Adapter Integration Tests

**Problem:** New repository methods aren't tested against real database.

**Solution:** Add integration tests with test database.

### Setup:

1. Create `docker-compose.test.yml` with PostgreSQL
2. Create test database initialization script
3. Add integration test module that:
   - Connects to test database
   - Runs migrations
   - Executes tests
   - Cleans up

### Tests:

```rust
#[tokio::test]
async fn agent_repo_find_top_by_simulator_elo() {
    let db = setup_test_db().await;
    let repo = PostgresAgentRepository::new(db);

    // Insert test agents with different ELOs
    // Query top agents
    // Verify ordering
}

#[tokio::test]
async fn agent_repo_find_top_by_antfarm_elo() { ... }

#[tokio::test]
async fn issue_repo_find_by_tier() { ... }

// ... etc for all repository methods
```

### Files to create:
- `tests/integration/mod.rs`
- `tests/integration/agent_repo.rs`
- `tests/integration/issue_repo.rs`
- `docker-compose.test.yml`

---

## Execution Order

1. **Renderer tests** (30 min) - Quick win, no refactoring needed
2. **Webhook implementation** (2 hr) - Important for actual functionality
3. **Leaderboard improvements** (1 hr) - User-facing value
4. **E2E tests + Generic AppState** (2 hr) - Enables proper testing
5. **Handler tests** (1 hr) - After E2E infrastructure exists
6. **ClickHouse decision** (30 min decision, 2 hr if implementing)
7. **Integration tests** (1 hr) - Requires docker setup

---

## Definition of Done

For each item:
- [ ] Implementation complete (no TODOs, no stubs)
- [ ] Unit tests written and passing
- [ ] Integration tests where applicable
- [ ] Clippy passes with no warnings
- [ ] Documentation updated if API changed
- [ ] Manually verified working
