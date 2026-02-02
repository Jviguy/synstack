-- ClickHouse Schema for SynStack
-- Execution traces, metrics, and analytics

-- ============================================================
-- EXECUTION TRACES (Simulator)
-- ============================================================

CREATE TABLE IF NOT EXISTS execution_traces (
    id UUID,
    submission_id UUID,
    agent_id UUID,
    issue_id UUID,

    -- Execution details
    stdout String,
    stderr String,
    exit_code Int32,
    duration_ms UInt64,

    -- Environment
    language LowCardinality(String),
    runtime_version String,

    -- Timestamps
    started_at DateTime,
    finished_at DateTime,
    created_at DateTime DEFAULT now()
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (agent_id, created_at)
TTL created_at + INTERVAL 1 YEAR;

-- ============================================================
-- COLLABORATION TRACES (Ant Farm)
-- ============================================================

CREATE TABLE IF NOT EXISTS collaboration_events (
    id UUID,
    project_id UUID,
    agent_id UUID,

    -- Event type
    event_type LowCardinality(String),  -- 'pr_opened', 'pr_merged', 'review_submitted', 'ticket_claimed', etc.

    -- References
    pr_id Nullable(UUID),
    ticket_id Nullable(UUID),

    -- Event data (flexible JSON)
    event_data String,  -- JSON blob

    -- Timestamps
    created_at DateTime DEFAULT now()
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (project_id, created_at);

-- ============================================================
-- AGENT METRICS (ELO history, activity)
-- ============================================================

CREATE TABLE IF NOT EXISTS agent_metrics (
    agent_id UUID,

    -- Snapshot time
    snapshot_at DateTime DEFAULT now(),

    -- ELO ratings
    simulator_elo Int32,
    antfarm_elo Int32,

    -- Activity counts (rolling 24h)
    submissions_24h UInt32,
    prs_merged_24h UInt32,
    reviews_given_24h UInt32,

    -- Cumulative stats
    total_submissions UInt64,
    total_accepted UInt64,
    total_prs_merged UInt64
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(snapshot_at)
ORDER BY (agent_id, snapshot_at)
TTL snapshot_at + INTERVAL 6 MONTH;

-- ============================================================
-- PROJECT HEALTH (Ant Farm)
-- ============================================================

CREATE TABLE IF NOT EXISTS project_health (
    project_id UUID,

    -- Snapshot time
    snapshot_at DateTime DEFAULT now(),

    -- Health metrics
    build_status LowCardinality(String),  -- 'passing', 'failing', 'unknown'
    open_tickets UInt32,
    active_contributors UInt32,
    prs_open UInt32,
    prs_merged_7d UInt32,
    commits_7d UInt32,

    -- Code metrics (if available)
    lines_of_code Nullable(UInt64),
    test_coverage Nullable(Float32)
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(snapshot_at)
ORDER BY (project_id, snapshot_at)
TTL snapshot_at + INTERVAL 1 YEAR;

-- ============================================================
-- API REQUEST LOGS
-- ============================================================

CREATE TABLE IF NOT EXISTS api_requests (
    -- Request info
    request_id UUID,
    agent_id Nullable(UUID),

    -- HTTP details
    method LowCardinality(String),
    path String,
    status_code UInt16,
    duration_ms UInt32,

    -- Client info
    user_agent String,
    ip_address String,

    -- Timestamps
    created_at DateTime DEFAULT now()
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(created_at)
ORDER BY (created_at)
TTL created_at + INTERVAL 30 DAY;
