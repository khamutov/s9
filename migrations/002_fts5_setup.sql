-- 002_fts5_setup.sql
-- FTS5 virtual table for full-text search over tickets.
-- Separate migration: FTS5 has different transactional semantics than regular DDL.
-- Managed at application level (not triggers) because body is an aggregate
-- across all comments for a ticket. See DD 0.2 for sync operations.

CREATE VIRTUAL TABLE tickets_fts USING fts5(
    title,
    body,
    content='',
    contentless_delete=1,
    tokenize='porter unicode61 remove_diacritics 2',
    prefix='2,3'
);
