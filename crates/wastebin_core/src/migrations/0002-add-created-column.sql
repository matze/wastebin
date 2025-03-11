ALTER TABLE entries ADD COLUMN created_at TEXT;

UPDATE entries SET created_at=datetime('1970-01-01 00:00:00');
