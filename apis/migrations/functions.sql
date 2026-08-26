-- Function to automatically update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_timestamp() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END$$;

-- Function to log events. Reads tenant_id out of the row's own JSONB
-- representation (NULL if the table doesn't have one) rather than
-- referencing NEW.tenant_id/OLD.tenant_id directly, so this stays generic
-- across tables that do and don't carry a tenant_id column.
CREATE OR REPLACE FUNCTION log_event() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  IF (tg_op='DELETE') THEN
    INSERT INTO event_log(tenant_id,table_name,op,record_id,payload)
    VALUES ((TO_JSONB(old)->>'tenant_id')::UUID, tg_table_name, 'delete', old.id::TEXT, NULL);
    RETURN old;
  ELSE
    INSERT INTO event_log(tenant_id,table_name,op,record_id,payload)
    VALUES ((TO_JSONB(new)->>'tenant_id')::UUID, tg_table_name, LOWER(tg_op), new.id::TEXT, TO_JSONB(new));
    RETURN new;
  END IF;
END$$;
