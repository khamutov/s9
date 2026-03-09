-- Add payload column to pending_notifications for storing event metadata as JSON.
-- Per DD 0.6 §14.1: stores old/new values, actor login for email templates.
ALTER TABLE pending_notifications ADD COLUMN payload TEXT;
