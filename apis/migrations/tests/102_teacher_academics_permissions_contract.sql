-- Teacher Academics permission contract checks. Run after migration 102.

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'teacher'
          AND deleted_at IS NULL
          AND (
              'academics:edit' = ANY(permissions)
              OR NOT ('academics:teach' = ANY(permissions))
          )
    ) THEN
        RAISE EXCEPTION 'a Teacher role can administer Academics or cannot perform assigned teaching work';
    END IF;
END;
$$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM roles
        WHERE key = 'teacher'
          AND deleted_at IS NULL
          AND permissions && ARRAY[
              'academics:create',
              'academics:delete',
              'academics:manage'
          ]::TEXT[]
    ) THEN
        RAISE EXCEPTION 'a Teacher role has academic administration authority';
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger
        WHERE tgname = 'zzzz_harden_new_tenant_teacher_academics'
          AND NOT tgisinternal
    ) THEN
        RAISE EXCEPTION 'new-tenant Teacher permission hardening is missing';
    END IF;
END;
$$;

SELECT 'Teacher Academics permission contract passed' AS result;
