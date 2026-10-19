-- Trusted installation bootstrap only; also safe to apply explicitly to an existing
-- veoveo database. Keep extension ownership separate from application/service roles.
\set ON_ERROR_STOP on
\connect veoveo
BEGIN;
DO $$
BEGIN
    IF current_user <> 'postgres' THEN
        RAISE EXCEPTION 'run extension enablement as trusted postgres';
    END IF;
END
$$;

CREATE SCHEMA IF NOT EXISTS _extensions AUTHORIZATION postgres;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT FROM pg_catalog.pg_namespace AS n
        JOIN pg_catalog.pg_roles AS r ON r.oid = n.nspowner
        WHERE n.nspname = '_extensions' AND r.rolname = 'postgres'
    ) THEN
        RAISE EXCEPTION '_extensions must already be owned by trusted postgres';
    END IF;
END
$$;
REVOKE ALL ON SCHEMA _extensions FROM PUBLIC;
GRANT USAGE ON SCHEMA _extensions TO PUBLIC;

CREATE EXTENSION IF NOT EXISTS vector WITH SCHEMA _extensions;
CREATE EXTENSION IF NOT EXISTS postgis WITH SCHEMA _extensions;
DO $$
BEGIN
    IF EXISTS (
        SELECT FROM pg_catalog.pg_extension AS e
        JOIN pg_catalog.pg_namespace AS n ON n.oid = e.extnamespace
        JOIN pg_catalog.pg_roles AS r ON r.oid = e.extowner
        WHERE e.extname IN ('vector', 'postgis') AND (n.nspname <> '_extensions' OR r.rolname <> 'postgres')
    ) THEN
        RAISE EXCEPTION 'existing vector/postgis must be postgres-owned in _extensions; inspect before changing';
    END IF;
    IF EXISTS (
        SELECT FROM pg_catalog.pg_extension
        WHERE (extname = 'vector' AND extversion <> '0.8.7')
           OR (extname = 'postgis' AND extversion <> '3.6.4')
    ) THEN
        RAISE EXCEPTION 'expected vector 0.8.7 and postgis 3.6.4; review extension upgrades explicitly';
    END IF;
END
$$;
COMMIT;

SELECT e.extname, e.extversion, n.nspname AS schema
FROM pg_catalog.pg_extension AS e
JOIN pg_catalog.pg_namespace AS n ON n.oid = e.extnamespace
WHERE e.extname IN ('vector', 'postgis')
ORDER BY e.extname;
