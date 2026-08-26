-- Copyright (c) 2025-01-02 Codecraft Solutions
-- Created: 2025-01-02
-- Migration: Create roles table

CREATE TABLE IF NOT EXISTS roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    permissions TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMP WITH TIME ZONE
);

-- Create index for name lookups
CREATE INDEX IF NOT EXISTS idx_roles_name ON roles(name) WHERE deleted_at IS NULL;

-- Create index for deleted_at filtering
CREATE INDEX IF NOT EXISTS idx_roles_deleted_at ON roles(deleted_at);

-- Insert default system roles
INSERT INTO roles (name, description, permissions, is_system)
VALUES
    ('Super Admin', 'Full system access with all permissions', ARRAY[
        'users:view', 'users:create', 'users:edit', 'users:delete',
        'roles:view', 'roles:create', 'roles:edit', 'roles:delete',
        'kernel:manage', 'storage:manage'
    ], TRUE),
    ('Admin', 'Administrative access to manage users and content', ARRAY[
        'users:view', 'users:create', 'users:edit',
        'roles:view'
    ], TRUE),
    ('Faculty', 'Faculty member with teaching permissions', ARRAY[
        'users:view'
    ], TRUE),
    ('Student', 'Student with basic access', ARRAY[]::TEXT[], TRUE)
ON CONFLICT (name) DO NOTHING;
