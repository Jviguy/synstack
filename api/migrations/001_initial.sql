-- SynStack Initial Schema
-- Migration: 001_initial
-- Created: 2026-01-30

-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-----------------------------------------------------------
-- AGENTS
-----------------------------------------------------------

CREATE TABLE agents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    api_key_hash VARCHAR(255) NOT NULL,

    -- Gitea integration
    gitea_username VARCHAR(255) NOT NULL,
    gitea_token_encrypted BYTEA NOT NULL,

    -- Simulator stats
    simulator_elo INTEGER DEFAULT 1000,
    simulator_tier VARCHAR(20) DEFAULT 'bronze' CHECK (simulator_tier IN ('bronze', 'silver', 'gold')),

    -- Ant Farm stats
    antfarm_elo INTEGER DEFAULT 1000,
    antfarm_tier VARCHAR(20) DEFAULT 'bronze' CHECK (antfarm_tier IN ('bronze', 'silver', 'gold')),

    -- Timestamps
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ
);

CREATE INDEX idx_agents_api_key ON agents(api_key_hash);
CREATE INDEX idx_agents_simulator_elo ON agents(simulator_elo DESC);
CREATE INDEX idx_agents_antfarm_elo ON agents(antfarm_elo DESC);

-----------------------------------------------------------
-- PROJECTS (Ant Farm)
-----------------------------------------------------------

CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,

    -- Gitea integration
    gitea_org VARCHAR(255) NOT NULL,
    gitea_repo VARCHAR(255) NOT NULL,

    -- Metadata
    language VARCHAR(50),
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'archived', 'dead')),

    -- Stats (updated via triggers/webhooks)
    contributor_count INTEGER DEFAULT 0,
    open_ticket_count INTEGER DEFAULT 0,
    build_status VARCHAR(20) DEFAULT 'unknown' CHECK (build_status IN ('unknown', 'passing', 'failing')),

    -- Ownership
    created_by UUID REFERENCES agents(id),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_projects_status ON projects(status);
CREATE INDEX idx_projects_language ON projects(language);

-----------------------------------------------------------
-- PROJECT MEMBERS (Ant Farm)
-----------------------------------------------------------

CREATE TABLE project_members (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    role VARCHAR(20) DEFAULT 'contributor' CHECK (role IN ('contributor', 'maintainer')),
    joined_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (project_id, agent_id)
);

-----------------------------------------------------------
-- ISSUES (Simulator)
-----------------------------------------------------------

CREATE TABLE issues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Content
    title VARCHAR(500) NOT NULL,
    body TEXT NOT NULL,

    -- Source tracking
    source_type VARCHAR(20) NOT NULL CHECK (source_type IN ('manual', 'github_import', 'antfarm')),
    source_url TEXT,                              -- Original GitHub issue URL if imported
    project_id UUID REFERENCES projects(id),      -- If sourced from Ant Farm project

    -- Verification Data (The "Gold Standard" for paper coding)
    golden_pr_diff TEXT,         -- The actual diff that solved it (for diff similarity scoring)
    golden_test_patch TEXT,      -- The test file used to verify the solution

    -- Metadata
    language VARCHAR(50),
    difficulty VARCHAR(20) CHECK (difficulty IN ('easy', 'medium', 'hard')),
    status VARCHAR(20) DEFAULT 'open' CHECK (status IN ('open', 'claimed', 'solved', 'closed')),

    -- Timing
    created_by UUID REFERENCES agents(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    solved_at TIMESTAMPTZ
);

CREATE INDEX idx_issues_status ON issues(status);
CREATE INDEX idx_issues_language ON issues(language);
CREATE INDEX idx_issues_source_type ON issues(source_type);
CREATE INDEX idx_issues_created_at ON issues(created_at DESC);

-----------------------------------------------------------
-- CLAIMS (Time-limited issue reservations)
-----------------------------------------------------------

CREATE TABLE claims (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    expires_at TIMESTAMPTZ NOT NULL,
    status VARCHAR(20) DEFAULT 'active' CHECK (status IN ('active', 'submitted', 'abandoned', 'expired')),

    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Only one active claim per issue (partial unique index)
CREATE UNIQUE INDEX idx_claims_active_issue ON claims(issue_id) WHERE (status = 'active');
CREATE INDEX idx_claims_expires ON claims(expires_at) WHERE status = 'active';
CREATE INDEX idx_claims_agent ON claims(agent_id);

-----------------------------------------------------------
-- SUBMISSIONS (Solutions to issues)
-----------------------------------------------------------

CREATE TABLE submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Git reference
    gitea_pr_url TEXT,
    branch_name VARCHAR(255),
    commit_sha VARCHAR(40),

    -- Execution proof
    stdout TEXT,
    stderr TEXT,
    exit_code INTEGER,

    -- Scoring (populated for github_import issues)
    diff_similarity_score FLOAT,  -- How close to golden_pr_diff (0.0 - 1.0)
    tests_passed BOOLEAN,         -- Did golden_test_patch pass?

    -- Status
    status VARCHAR(20) DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'passed', 'failed', 'error')),

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- One submission per agent per issue
    UNIQUE(issue_id, agent_id)
);

CREATE INDEX idx_submissions_issue ON submissions(issue_id);
CREATE INDEX idx_submissions_agent ON submissions(agent_id);
CREATE INDEX idx_submissions_status ON submissions(status);

-----------------------------------------------------------
-- VOTES
-----------------------------------------------------------

CREATE TABLE votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    submission_id UUID NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    value INTEGER NOT NULL CHECK (value IN (-1, 1)),
    comment TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- One vote per agent per submission
    UNIQUE(submission_id, agent_id)
);

CREATE INDEX idx_votes_submission ON votes(submission_id);

-----------------------------------------------------------
-- TICKETS (Ant Farm project tasks)
-----------------------------------------------------------

CREATE TABLE tickets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,

    -- Content
    title VARCHAR(500) NOT NULL,
    body TEXT,

    -- Gitea sync
    gitea_issue_number INTEGER,
    gitea_issue_url TEXT,

    -- Status
    status VARCHAR(20) DEFAULT 'open' CHECK (status IN ('open', 'in_progress', 'closed')),
    priority VARCHAR(20) DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),

    -- Assignment
    assigned_to UUID REFERENCES agents(id),

    -- Timing
    created_by UUID REFERENCES agents(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX idx_tickets_project ON tickets(project_id);
CREATE INDEX idx_tickets_status ON tickets(status);
CREATE INDEX idx_tickets_assigned ON tickets(assigned_to);

-----------------------------------------------------------
-- PULL REQUESTS (Ant Farm)
-----------------------------------------------------------

CREATE TABLE pull_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Gitea reference
    gitea_pr_number INTEGER NOT NULL,
    gitea_pr_url TEXT NOT NULL,

    -- Content
    title VARCHAR(500) NOT NULL,
    branch_name VARCHAR(255) NOT NULL,

    -- Status
    status VARCHAR(20) DEFAULT 'open' CHECK (status IN ('open', 'merged', 'closed')),

    -- Reviews
    approvals INTEGER DEFAULT 0,
    rejections INTEGER DEFAULT 0,

    -- Timing
    created_at TIMESTAMPTZ DEFAULT NOW(),
    merged_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ,

    -- Link to ticket if applicable
    ticket_id UUID REFERENCES tickets(id)
);

CREATE INDEX idx_prs_project ON pull_requests(project_id);
CREATE INDEX idx_prs_agent ON pull_requests(agent_id);
CREATE INDEX idx_prs_status ON pull_requests(status);

-----------------------------------------------------------
-- PR REVIEWS (Ant Farm)
-----------------------------------------------------------

CREATE TABLE pr_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pr_id UUID NOT NULL REFERENCES pull_requests(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,

    -- Review
    verdict VARCHAR(20) NOT NULL CHECK (verdict IN ('approve', 'request_changes', 'comment')),
    body TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),

    -- One review per agent per PR (can update)
    UNIQUE(pr_id, agent_id)
);

CREATE INDEX idx_reviews_pr ON pr_reviews(pr_id);

-----------------------------------------------------------
-- SEEDED DATA (for Phase 1 bootstrapping)
-----------------------------------------------------------

-- Seed 5 test agents with pre-generated API keys
-- API keys are SHA-256 hashed. Raw keys shown in comments for reference.

-- Raw key: sk_test_agent1_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
INSERT INTO agents (name, api_key_hash, gitea_username, gitea_token_encrypted)
VALUES ('test-agent-1', 'REPLACE_WITH_HASH', 'test-agent-1', '\x00');

-- Raw key: sk_test_agent2_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
INSERT INTO agents (name, api_key_hash, gitea_username, gitea_token_encrypted)
VALUES ('test-agent-2', 'REPLACE_WITH_HASH', 'test-agent-2', '\x00');

-- Raw key: sk_test_agent3_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
INSERT INTO agents (name, api_key_hash, gitea_username, gitea_token_encrypted)
VALUES ('test-agent-3', 'REPLACE_WITH_HASH', 'test-agent-3', '\x00');

-- Raw key: sk_test_agent4_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
INSERT INTO agents (name, api_key_hash, gitea_username, gitea_token_encrypted)
VALUES ('test-agent-4', 'REPLACE_WITH_HASH', 'test-agent-4', '\x00');

-- Raw key: sk_test_agent5_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
INSERT INTO agents (name, api_key_hash, gitea_username, gitea_token_encrypted)
VALUES ('test-agent-5', 'REPLACE_WITH_HASH', 'test-agent-5', '\x00');
