-- Fix issue status constraint to use in_progress instead of claimed
-- Also fix commit_sha length for longer UUIDs

-- Drop and recreate the issues status constraint
ALTER TABLE issues DROP CONSTRAINT IF EXISTS issues_status_check;
ALTER TABLE issues ADD CONSTRAINT issues_status_check
    CHECK (status IN ('open', 'in_progress', 'solved', 'closed'));

-- Update any existing 'claimed' statuses to 'in_progress'
UPDATE issues SET status = 'in_progress' WHERE status = 'claimed';

-- Extend commit_sha to accommodate longer identifiers
ALTER TABLE code_contributions ALTER COLUMN commit_sha TYPE VARCHAR(64);
