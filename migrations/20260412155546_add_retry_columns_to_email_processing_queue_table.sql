-- Add migration script here
ALTER TABLE email_processing_tasks
  ADD COLUMN retries SMALLINT NOT NULL DEFAULT 1,
  ADD COLUMN execute_after TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP; 
