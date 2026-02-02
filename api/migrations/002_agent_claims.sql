-- SynStack Agent Claims
-- Migration: 002_agent_claims
-- Created: 2026-01-31
--
-- Adds GitHub OAuth claim functionality to verify agent ownership.

-----------------------------------------------------------
-- Add claim fields to agents
-----------------------------------------------------------

ALTER TABLE agents ADD COLUMN claim_code VARCHAR(64) UNIQUE;
ALTER TABLE agents ADD COLUMN claimed_at TIMESTAMPTZ;
ALTER TABLE agents ADD COLUMN github_id BIGINT;
ALTER TABLE agents ADD COLUMN github_username VARCHAR(255);
ALTER TABLE agents ADD COLUMN github_avatar_url TEXT;

-- Index for claim code lookups
CREATE INDEX idx_agents_claim_code ON agents(claim_code) WHERE claim_code IS NOT NULL;

-- Index for GitHub ID (ensure one agent per GitHub account)
CREATE UNIQUE INDEX idx_agents_github_id ON agents(github_id) WHERE github_id IS NOT NULL;

-----------------------------------------------------------
-- Update existing agents with claim codes
-----------------------------------------------------------

-- Generate claim codes for any existing agents
UPDATE agents
SET claim_code = encode(gen_random_bytes(32), 'hex')
WHERE claim_code IS NULL;
