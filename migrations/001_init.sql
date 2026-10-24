-- Phase 1 initial schema. Field names align with exports/grouped.json.

CREATE TABLE block (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    split TEXT NOT NULL CHECK (split IN ('upper_lower', 'full_body', 'anterior_posterior', 'other')),
    label TEXT,
    started_on TEXT
);

CREATE TABLE cycle (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_id INTEGER NOT NULL REFERENCES block(id) ON DELETE CASCADE,
    week INTEGER NOT NULL,
    title TEXT,
    note_id TEXT,
    started_on TEXT,
    date_precision TEXT NOT NULL DEFAULT 'unknown'
        CHECK (date_precision IN ('exact', 'week', 'unknown')),
    override INTEGER NOT NULL DEFAULT 0 CHECK (override IN (0, 1)),
    source TEXT NOT NULL DEFAULT 'app' CHECK (source IN ('app', 'notes_import')),
    source_hash TEXT,
    UNIQUE (block_id, week)
);

CREATE TABLE session (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cycle_id INTEGER NOT NULL REFERENCES cycle(id) ON DELETE CASCADE,
    day_number INTEGER,
    day_label TEXT,
    comment TEXT,
    header_raw TEXT,
    implicit INTEGER NOT NULL DEFAULT 0 CHECK (implicit IN (0, 1)),
    "index" INTEGER NOT NULL DEFAULT 0,
    notes TEXT NOT NULL DEFAULT '[]',
    performed_on TEXT,
    date_precision TEXT NOT NULL DEFAULT 'unknown'
        CHECK (date_precision IN ('exact', 'week', 'unknown')),
    source TEXT NOT NULL DEFAULT 'app' CHECK (source IN ('app', 'notes_import'))
);

CREATE TABLE variant (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slug TEXT NOT NULL UNIQUE,
    display TEXT NOT NULL,
    family TEXT,
    gym TEXT,
    status TEXT NOT NULL DEFAULT 'confirmed'
        CHECK (status IN ('confirmed', 'suggested')),
    joint_action TEXT,
    equipment TEXT,
    load_unit TEXT NOT NULL DEFAULT 'lb',
    default_settings TEXT
);

CREATE TABLE alias (
    normalized TEXT PRIMARY KEY,
    variant_id INTEGER NOT NULL REFERENCES variant(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'confirmed'
        CHECK (status IN ('confirmed', 'suggested'))
);

CREATE TABLE entry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    variant_id INTEGER REFERENCES variant(id) ON DELETE SET NULL,
    position INTEGER NOT NULL,
    tracked INTEGER NOT NULL DEFAULT 1 CHECK (tracked IN (0, 1)),
    name TEXT,
    name_raw TEXT NOT NULL,
    qualifiers TEXT NOT NULL DEFAULT '[]',
    untracked_sets INTEGER,
    settings TEXT,
    comment TEXT,
    raw TEXT,
    excluded INTEGER NOT NULL DEFAULT 0 CHECK (excluded IN (0, 1))
);

CREATE TABLE set_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id INTEGER NOT NULL REFERENCES entry(id) ON DELETE CASCADE,
    set_number INTEGER NOT NULL,
    weight REAL,
    weight_inferred INTEGER NOT NULL DEFAULT 0 CHECK (weight_inferred IN (0, 1)),
    weight_note TEXT,
    reps REAL,
    rir_low REAL,
    rir_high REAL,
    logged INTEGER NOT NULL DEFAULT 1 CHECK (logged IN (0, 1)),
    comment TEXT,
    raw TEXT
);

CREATE INDEX idx_cycle_block ON cycle(block_id);
CREATE INDEX idx_session_cycle ON session(cycle_id);
CREATE INDEX idx_entry_session ON entry(session_id);
CREATE INDEX idx_entry_variant ON entry(variant_id);
CREATE INDEX idx_set_entry ON set_log(entry_id);
CREATE INDEX idx_alias_variant ON alias(variant_id);
