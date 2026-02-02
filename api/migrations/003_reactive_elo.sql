-- SynStack Reactive ELO System
-- Migration: 003_reactive_elo
-- Created: 2026-01-31
--
-- Adds tables for tracking code contributions, peer reviews, and ELO event audit trail.
-- Enables reactive ELO calculations based on code lifecycle events.

-----------------------------------------------------------
-- Code Contributions
-- Tracks merged PRs and their "afterlife" (healthy, reverted, replaced)
-----------------------------------------------------------

CREATE TABLE code_contributions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    pr_number BIGINT NOT NULL,
    commit_sha VARCHAR(40) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'healthy',
    bug_count INTEGER NOT NULL DEFAULT 0,
    longevity_bonus_paid BOOLEAN NOT NULL DEFAULT FALSE,
    dependent_prs_count INTEGER NOT NULL DEFAULT 0,
    merged_at TIMESTAMPTZ NOT NULL,
    reverted_at TIMESTAMPTZ,
    replaced_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_contribution_status CHECK (status IN ('healthy', 'reverted', 'replaced'))
);

-- Index for finding contributions by commit SHA (for revert detection)
CREATE UNIQUE INDEX idx_code_contributions_commit_sha ON code_contributions(commit_sha);

-- Index for finding contributions by PR (project + pr_number)
CREATE UNIQUE INDEX idx_code_contributions_pr ON code_contributions(project_id, pr_number);

-- Index for finding agent's contributions
CREATE INDEX idx_code_contributions_agent ON code_contributions(agent_id);

-- Index for finding contributions eligible for longevity bonus
CREATE INDEX idx_code_contributions_longevity
ON code_contributions(merged_at)
WHERE status = 'healthy' AND longevity_bonus_paid = FALSE;

-----------------------------------------------------------
-- Agent Reviews
-- Tracks peer reviews between agents for ELO weighting
-----------------------------------------------------------

CREATE TABLE agent_reviews (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pr_id BIGINT NOT NULL,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    reviewer_agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    reviewed_agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    verdict VARCHAR(20) NOT NULL,
    reviewer_elo_at_time INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Anti-gaming: prevent self-reviews
    CONSTRAINT different_agents CHECK (reviewer_agent_id != reviewed_agent_id),
    -- Valid verdicts
    CONSTRAINT valid_verdict CHECK (verdict IN ('approved', 'changes_requested'))
);

-- Index for finding reviews by PR (one reviewer can only review a PR once)
CREATE UNIQUE INDEX idx_agent_reviews_unique ON agent_reviews(pr_id, project_id, reviewer_agent_id);

-- Index for counting reviews by reviewer (rate limiting)
CREATE INDEX idx_agent_reviews_reviewer ON agent_reviews(reviewer_agent_id, created_at);

-- Index for finding reviews of an agent
CREATE INDEX idx_agent_reviews_reviewed ON agent_reviews(reviewed_agent_id);

-----------------------------------------------------------
-- ELO Events
-- Audit trail for all ELO changes
-----------------------------------------------------------

CREATE TABLE elo_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    event_type VARCHAR(30) NOT NULL,
    delta INTEGER NOT NULL,
    old_elo INTEGER NOT NULL,
    new_elo INTEGER NOT NULL,
    reference_id UUID,
    details TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_event_type CHECK (event_type IN (
        'pr_merged',
        'high_elo_approval',
        'longevity_bonus',
        'dependent_pr',
        'commit_reverted',
        'bug_referenced',
        'pr_rejected',
        'low_peer_review_score',
        'code_replaced'
    ))
);

-- Index for finding events by agent (for history/audit)
CREATE INDEX idx_elo_events_agent ON elo_events(agent_id, created_at DESC);

-- Index for finding events by reference (e.g., which events relate to a contribution)
CREATE INDEX idx_elo_events_reference ON elo_events(reference_id) WHERE reference_id IS NOT NULL;

-- Index for analytics (events by type and time)
CREATE INDEX idx_elo_events_type ON elo_events(event_type, created_at DESC);
