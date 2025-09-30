-- CREATE TABLE IF NOT EXISTS statements
DO $$ BEGIN
    CREATE TYPE APP_STATE AS ENUM ('Uninitialized','SchoolConfigured','Ready');
EXCEPTION
    WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS system_state (
  id             TEXT PRIMARY KEY DEFAULT 'singleton',
  state          APP_STATE NOT NULL DEFAULT 'Uninitialized',
  kernel_lock BOOLEAN NOT NULL DEFAULT FALSE,
  deleted_at     TIMESTAMP WITH TIME ZONE,
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
  logo_light_url TEXT,
  logo_dark_url  TEXT,
  deleted_at     TIMESTAMP WITH TIME ZONE,
  created_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at     TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Enforce single-row
CREATE UNIQUE INDEX IF NOT EXISTS one_school_only ON school_profile((TRUE)) WHERE id='singleton';

CREATE TABLE IF NOT EXISTS users(
  id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  email         TEXT UNIQUE NOT NULL CHECK (email = LOWER(email)),
  password_hash TEXT NOT NULL,
  full_name     TEXT NOT NULL,
  phone         TEXT,
  is_active     BOOLEAN NOT NULL DEFAULT TRUE,
  roles         TEXT[] NOT NULL DEFAULT '{}'::TEXT[],
  deleted_at    TIMESTAMP WITH TIME ZONE,
  created_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_users_email_lower ON users(LOWER(email));

CREATE TABLE IF NOT EXISTS event_log(
  id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  table_name  TEXT NOT NULL,
  op          TEXT NOT NULL CHECK (op IN ('insert','update','delete')),
  record_id   TEXT NOT NULL,
  payload     JSONB,
  occurred_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  deleted_at  TIMESTAMP WITH TIME ZONE,
  created_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at  TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
