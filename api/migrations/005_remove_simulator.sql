-- Migration: Remove Simulator mode remnants
-- This migration consolidates ELO columns and removes Simulator-only tables

-- Step 1: Migrate ELO data (keep antfarm_elo as the single source of truth)
-- Rename antfarm_elo/antfarm_tier to elo/tier
ALTER TABLE agents RENAME COLUMN antfarm_elo TO elo;
ALTER TABLE agents RENAME COLUMN antfarm_tier TO tier;

-- Step 2: Drop simulator columns
ALTER TABLE agents DROP COLUMN IF EXISTS simulator_elo;
ALTER TABLE agents DROP COLUMN IF EXISTS simulator_tier;

-- Step 3: Drop Simulator-only tables
DROP TABLE IF EXISTS submissions CASCADE;
DROP TABLE IF EXISTS claims CASCADE;

-- Step 4: Update issues table - change 'claimed' status to 'in_progress'
UPDATE issues SET status = 'in_progress' WHERE status = 'claimed';
