-- Run as an existing non-superuser service login in veoveo. Supply its schema with
-- psql --set=service_schema=time (or artifact/media/hub). All test DDL rolls back.
\set ON_ERROR_STOP on
\if :{?service_schema}
\else
    \echo 'required psql variable: service_schema'
    \quit 1
\endif
BEGIN;
SELECT current_database() = 'veoveo'
       AND NOT (SELECT rolsuper FROM pg_catalog.pg_roles WHERE rolname = current_user)
       AND EXISTS (
           SELECT FROM pg_catalog.pg_namespace
           WHERE nspname = :'service_schema'
             AND pg_catalog.pg_has_role(current_user, nspowner, 'USAGE')
       ) AS admitted
\gset
\if :admitted
\else
    \echo 'connect to veoveo as the non-superuser owner/member of service_schema'
    \quit 1
\endif
SELECT current_user AS verification_role, :'service_schema' AS service_schema;

DO $$
BEGIN
    IF NOT pg_catalog.has_schema_privilege(current_user, '_extensions', 'USAGE')
       OR pg_catalog.has_schema_privilege(current_user, '_extensions', 'CREATE') THEN
        RAISE EXCEPTION 'extension schema must allow USAGE and deny CREATE';
    END IF;
    IF NOT pg_catalog.has_table_privilege(current_user, '_extensions.spatial_ref_sys', 'SELECT')
       OR pg_catalog.has_table_privilege(current_user, '_extensions.spatial_ref_sys', 'INSERT')
       OR pg_catalog.has_table_privilege(current_user, '_extensions.spatial_ref_sys', 'UPDATE')
       OR pg_catalog.has_table_privilege(current_user, '_extensions.spatial_ref_sys', 'DELETE') THEN
        RAISE EXCEPTION 'spatial reference data must be readable, not application-writable';
    END IF;
    BEGIN
        CREATE TABLE _extensions._extension_verification_forbidden (id integer);
    EXCEPTION WHEN insufficient_privilege THEN
        RETURN;
    END;
    RAISE EXCEPTION 'unexpected CREATE permission in _extensions';
END
$$;

CREATE TABLE :"service_schema"._extension_verification (
    id integer PRIMARY KEY,
    embedding _extensions.vector(3) NOT NULL,
    location _extensions.geometry(Point, 4326) NOT NULL
);
INSERT INTO :"service_schema"._extension_verification VALUES
    (1, '[1,2,3]', _extensions.ST_SetSRID(_extensions.ST_MakePoint(0, 0), 4326)),
    (2, '[4,6,3]', _extensions.ST_SetSRID(_extensions.ST_MakePoint(1, 1), 4326));
CREATE INDEX _extension_verification_hnsw
    ON :"service_schema"._extension_verification
    USING hnsw (embedding _extensions.vector_l2_ops);
CREATE INDEX _extension_verification_gist
    ON :"service_schema"._extension_verification USING gist (location);

DO $$
DECLARE
    projected _extensions.geometry;
BEGIN
    IF '[1,2,3]'::_extensions.vector OPERATOR(_extensions.<->)
       '[4,6,3]'::_extensions.vector <> 5 THEN
        RAISE EXCEPTION 'vector L2 distance differs from exact expected 5';
    END IF;
    projected := _extensions.ST_Transform(
        _extensions.ST_SetSRID(_extensions.ST_MakePoint(1, 1), 4326), 3857);
    IF _extensions.ST_SRID(projected) <> 3857
       OR abs(_extensions.ST_X(projected) - 111319.49079327357) > 1e-6
       OR abs(_extensions.ST_Y(projected) - 111325.1428663851) > 1e-6 THEN
        RAISE EXCEPTION 'EPSG:4326 to EPSG:3857 coordinate transform differs';
    END IF;
END
$$;

-- Force the small fixture's index plans; this proves index usability, not speed
-- or approximate-search recall. Retain both EXPLAIN outputs in the check receipt.
SET LOCAL enable_seqscan = off;
EXPLAIN (COSTS OFF)
SELECT id FROM :"service_schema"._extension_verification
ORDER BY embedding OPERATOR(_extensions.<->) '[1,2,3]'::_extensions.vector LIMIT 1;
SELECT id = 1 AS nearest_matches FROM :"service_schema"._extension_verification
ORDER BY embedding OPERATOR(_extensions.<->) '[1,2,3]'::_extensions.vector LIMIT 1
\gset
EXPLAIN (COSTS OFF)
SELECT id FROM :"service_schema"._extension_verification
WHERE location OPERATOR(_extensions.&&) _extensions.ST_MakeEnvelope(-0.5, -0.5, 0.5, 0.5, 4326);
SELECT count(*) = 1 AND min(id) = 1 AS spatial_matches
FROM :"service_schema"._extension_verification
WHERE location OPERATOR(_extensions.&&) _extensions.ST_MakeEnvelope(-0.5, -0.5, 0.5, 0.5, 4326)
\gset
\if :nearest_matches
\else
    \echo 'HNSW nearest-neighbor result differs'
    \quit 1
\endif
\if :spatial_matches
\else
    \echo 'GiST spatial result differs'
    \quit 1
\endif
ROLLBACK;
\echo 'extension service-role verification passed; fixture DDL rolled back'
