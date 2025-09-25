-- CREATE TABLE IF NOT EXISTS statements
CREATE TYPE APP_STATE AS ENUM ('Uninitialized','SchoolConfigured','Ready');

CREATE TABLE IF NOT EXISTS system_state (
  id             TEXT PRIMARY KEY DEFAULT 'singleton',
  state          APP_STATE NOT NULL DEFAULT 'Uninitialized',
  bootstrap_lock BOOLEAN NOT NULL DEFAULT FALSE,
  created_at     TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
  updated_at     TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
INSERT INTO system_state(id) VALUES ('singleton') ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS school_profile (
  id             TEXT PRIMARY KEY DEFAULT 'singleton',
  name           TEXT NOT NULL,
  legal_name     TEXT,
  emap_code      TEXT,
  phone          TEXT,
  email          TEXT,
  address_line1  TEXT,
  address_line2  TEXT,
  city           TEXT,
  province       TEXT,
  country        TEXT DEFAULT 'Zimbabwe',
  timezone       TEXT DEFAULT 'Africa/Harare',
  locale         TEXT DEFAULT 'en-ZW',
  logo_light_key TEXT,
  logo_dark_key  TEXT,
  created_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Enforce single-row
CREATE UNIQUE INDEX IF NOT EXISTS one_school_only ON school_profile((TRUE)) WHERE id='singleton';

CREATE TABLE IF NOT EXISTS users(
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email         CITEXT UNIQUE NOT NULL,
  password_hash TEXT NOT NULL,
  full_name     TEXT NOT NULL,
  phone         TEXT,
  is_active     BOOLEAN NOT NULL DEFAULT TRUE,
  roles         TEXT[] NOT NULL DEFAULT '{}'::TEXT[],
  created_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX ON users((LOWER(email)));

create table event_log(
  id          uuid primary key default gen_random_uuid(),
  table_name  text not null,
  op          text not null check (op in ('insert','update','delete')),
  record_id   text not null,
  payload     jsonb,
  occurred_at timestamptz not null default now()
);
