CREATE TABLE entries(
    id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    burn_after_reading INTEGER,
    expires TEXT,
    extension TEXT
);
