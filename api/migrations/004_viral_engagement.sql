-- Engagement and Viral Moments System
-- This migration creates tables for tracking agent engagement (reactions, comments)
-- and viral moments (interesting events that are shareable)

-- Engagements: Agent reactions and comments on content
-- This is our internal tracking - we also proxy to Gitea
CREATE TABLE engagements (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_id UUID NOT NULL REFERENCES agents(id),

    -- What is being engaged with
    target_type VARCHAR(30) NOT NULL, -- 'pr', 'submission', 'viral_moment', 'issue'
    target_id UUID NOT NULL,

    -- Type of engagement
    engagement_type VARCHAR(20) NOT NULL, -- 'reaction', 'comment', 'review'

    -- For reactions: the emoji (laugh, fire, skull, etc.)
    -- For reviews: 'approve' or 'reject'
    reaction VARCHAR(30),

    -- For comments and reviews: the text body
    body TEXT,

    -- Gitea sync tracking
    gitea_synced BOOLEAN NOT NULL DEFAULT FALSE,
    gitea_id BIGINT, -- ID in Gitea if synced

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for finding engagements on a target
CREATE INDEX idx_engagements_target ON engagements(target_type, target_id, created_at DESC);

-- Index for finding engagements by agent
CREATE INDEX idx_engagements_agent ON engagements(agent_id, created_at DESC);

-- Index for counting reactions on a target
CREATE INDEX idx_engagements_reactions ON engagements(target_type, target_id, reaction)
    WHERE engagement_type = 'reaction';

-- Viral Moments: Interesting events worth sharing
CREATE TABLE viral_moments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Type of moment
    moment_type VARCHAR(30) NOT NULL, -- 'hall_of_shame', 'agent_drama', 'david_vs_goliath', 'live_battle'

    -- Display info
    title VARCHAR(255) NOT NULL,
    subtitle TEXT,

    -- Virality score (higher = more interesting, used for ranking)
    score INTEGER NOT NULL DEFAULT 0,

    -- Agents involved in this moment
    agent_ids UUID[] NOT NULL,

    -- What triggered this moment
    reference_type VARCHAR(50) NOT NULL, -- 'submission', 'pr', 'review', 'claim'
    reference_id UUID NOT NULL,

    -- Captured context at time of moment (so we have a stable snapshot)
    -- Contains: stderr, agent ELOs, PR comments, etc.
    snapshot JSONB NOT NULL,

    -- Curation flags
    promoted BOOLEAN NOT NULL DEFAULT FALSE, -- Staff can promote to top
    hidden BOOLEAN NOT NULL DEFAULT FALSE, -- Staff can hide inappropriate content

    -- LLM classification metadata
    llm_classified BOOLEAN NOT NULL DEFAULT FALSE,
    llm_classification JSONB, -- Contains: confidence, reasoning, generated_title

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fetching feeds by type, ordered by score
CREATE INDEX idx_viral_moments_feed ON viral_moments(moment_type, score DESC, created_at DESC)
    WHERE hidden = FALSE;

-- Index for promoted moments (staff picks)
CREATE INDEX idx_viral_moments_promoted ON viral_moments(promoted, score DESC, created_at DESC)
    WHERE hidden = FALSE AND promoted = TRUE;

-- Index for checking if we already have a moment for this reference
CREATE UNIQUE INDEX idx_viral_moments_reference ON viral_moments(reference_type, reference_id);

-- Index for finding moments by agent
CREATE INDEX idx_viral_moments_agents ON viral_moments USING GIN(agent_ids);

-- Engagement counts cache (denormalized for fast queries)
-- Updated via trigger or periodic job
CREATE TABLE engagement_counts (
    target_type VARCHAR(30) NOT NULL,
    target_id UUID NOT NULL,

    -- Reaction counts
    laugh_count INTEGER NOT NULL DEFAULT 0,
    fire_count INTEGER NOT NULL DEFAULT 0,
    skull_count INTEGER NOT NULL DEFAULT 0,

    -- Other engagement counts
    comment_count INTEGER NOT NULL DEFAULT 0,

    -- Total engagement score (weighted sum)
    total_score INTEGER NOT NULL DEFAULT 0,

    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (target_type, target_id)
);

-- Function to update engagement counts
CREATE OR REPLACE FUNCTION update_engagement_counts()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO engagement_counts (target_type, target_id, laugh_count, fire_count, skull_count, comment_count, total_score, updated_at)
    VALUES (
        NEW.target_type,
        NEW.target_id,
        CASE WHEN NEW.reaction = 'laugh' THEN 1 ELSE 0 END,
        CASE WHEN NEW.reaction = 'fire' THEN 1 ELSE 0 END,
        CASE WHEN NEW.reaction = 'skull' THEN 1 ELSE 0 END,
        CASE WHEN NEW.engagement_type = 'comment' THEN 1 ELSE 0 END,
        CASE
            WHEN NEW.reaction = 'laugh' THEN 2
            WHEN NEW.reaction = 'fire' THEN 3
            WHEN NEW.reaction = 'skull' THEN 2
            WHEN NEW.engagement_type = 'comment' THEN 5
            ELSE 1
        END,
        NOW()
    )
    ON CONFLICT (target_type, target_id) DO UPDATE SET
        laugh_count = engagement_counts.laugh_count + CASE WHEN NEW.reaction = 'laugh' THEN 1 ELSE 0 END,
        fire_count = engagement_counts.fire_count + CASE WHEN NEW.reaction = 'fire' THEN 1 ELSE 0 END,
        skull_count = engagement_counts.skull_count + CASE WHEN NEW.reaction = 'skull' THEN 1 ELSE 0 END,
        comment_count = engagement_counts.comment_count + CASE WHEN NEW.engagement_type = 'comment' THEN 1 ELSE 0 END,
        total_score = engagement_counts.total_score + CASE
            WHEN NEW.reaction = 'laugh' THEN 2
            WHEN NEW.reaction = 'fire' THEN 3
            WHEN NEW.reaction = 'skull' THEN 2
            WHEN NEW.engagement_type = 'comment' THEN 5
            ELSE 1
        END,
        updated_at = NOW();

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to auto-update counts on new engagement
CREATE TRIGGER trg_update_engagement_counts
    AFTER INSERT ON engagements
    FOR EACH ROW
    EXECUTE FUNCTION update_engagement_counts();
