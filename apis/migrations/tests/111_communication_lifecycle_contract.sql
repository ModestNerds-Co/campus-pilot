-- Communication review, publication, cancellation, and evidence contract. All records roll back.

BEGIN;

DO $$
DECLARE
    test_tenant_id UUID;
    test_user_id UUID;
    test_announcement_id UUID := gen_random_uuid();
    test_delivery_id UUID := gen_random_uuid();
BEGIN
    SELECT account.tenant_id, account.id INTO test_tenant_id, test_user_id
      FROM users AS account
     WHERE account.deleted_at IS NULL AND account.is_active
     ORDER BY account.created_at, account.id LIMIT 1;

    IF test_user_id IS NULL THEN
        RAISE EXCEPTION 'Communication lifecycle contract requires one active account';
    END IF;

    INSERT INTO communication_announcements (
        id, tenant_id, title, body, priority, created_by
    ) VALUES (
        test_announcement_id, test_tenant_id, 'Communication contract',
        'Communication contract message', 'normal', test_user_id
    );

    INSERT INTO communication_audience_targets (
        tenant_id, announcement_id, target_kind, target_id, label_snapshot
    ) VALUES (
        test_tenant_id, test_announcement_id, 'individual', test_user_id,
        'Communication contract account'
    );

    INSERT INTO communication_deliveries (
        id, tenant_id, announcement_id, recipient_user_id, recipient_name_snapshot
    ) VALUES (
        test_delivery_id, test_tenant_id, test_announcement_id, test_user_id,
        'Communication contract account'
    );

    UPDATE communication_announcements
       SET status='submitted', submitted_by=test_user_id, submitted_at=NOW(),
           recipient_fingerprint=REPEAT('a', 64), version=version+1
     WHERE tenant_id=test_tenant_id AND id=test_announcement_id;

    BEGIN
        UPDATE communication_audience_targets
           SET label_snapshot='Changed audience'
         WHERE tenant_id=test_tenant_id AND announcement_id=test_announcement_id;
        RAISE EXCEPTION 'Reviewed audience accepted a mutation';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('draft' IN SQLERRM)=0 THEN RAISE; END IF;
    END;

    UPDATE communication_deliveries
       SET status='delivered', delivered_at=NOW()
     WHERE tenant_id=test_tenant_id AND id=test_delivery_id;

    UPDATE communication_announcements
       SET status='published', published_by=test_user_id, published_at=NOW(),
           version=version+1
     WHERE tenant_id=test_tenant_id AND id=test_announcement_id;

    UPDATE communication_deliveries
       SET read_at=NOW()
     WHERE tenant_id=test_tenant_id AND id=test_delivery_id;

    BEGIN
        UPDATE communication_deliveries
           SET read_at=read_at + INTERVAL '1 second'
         WHERE tenant_id=test_tenant_id AND id=test_delivery_id;
        RAISE EXCEPTION 'Communication delivery accepted a second read receipt';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('one read receipt' IN SQLERRM)=0 THEN RAISE; END IF;
    END;

    BEGIN
        DELETE FROM communication_deliveries
         WHERE tenant_id=test_tenant_id AND id=test_delivery_id;
        RAISE EXCEPTION 'Communication delivery evidence accepted deletion';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('cannot be deleted' IN SQLERRM)=0 THEN RAISE; END IF;
    END;

    UPDATE communication_announcements
       SET status='cancelled', cancelled_by=test_user_id, cancelled_at=NOW(),
           cancellation_reason='Published in error', version=version+1
     WHERE tenant_id=test_tenant_id AND id=test_announcement_id;

    IF NOT EXISTS (
        SELECT 1 FROM communication_announcements
         WHERE tenant_id=test_tenant_id AND id=test_announcement_id
           AND status='cancelled' AND cancellation_reason='Published in error'
    ) OR NOT EXISTS (
        SELECT 1 FROM communication_deliveries
         WHERE tenant_id=test_tenant_id AND id=test_delivery_id
           AND status='delivered' AND read_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'Communication lifecycle evidence was not retained';
    END IF;

    BEGIN
        DELETE FROM communication_announcements
         WHERE tenant_id=test_tenant_id AND id=test_announcement_id;
        RAISE EXCEPTION 'Communication announcement accepted hard deletion';
    EXCEPTION WHEN OTHERS THEN
        IF POSITION('hard-deleted' IN SQLERRM)=0 THEN RAISE; END IF;
    END;
END;
$$;

ROLLBACK;

SELECT 'Communication lifecycle contract passed' AS result;
