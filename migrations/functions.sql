CREATE OR REPLACE FUNCTION log_event() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  IF (tg_op='DELETE') THEN
    INSERT INTO event_log(table_name,op,record_id,payload)
    VALUES (tg_table_name,'delete', old.id::TEXT, NULL);
    RETURN old;
  ELSE
    INSERT INTO event_log(table_name,op,record_id,payload)
    VALUES (tg_table_name, LOWER(tg_op), new.id::TEXT, TO_JSONB(new));
    RETURN new;
  END IF;
END$$;
