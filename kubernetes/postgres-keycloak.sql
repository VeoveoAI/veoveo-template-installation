-- Keycloak's login and database, for an installation that runs Keycloak in the cluster
-- (kubernetes/keycloak.yaml). On a new installation, add this file to the
-- veoveo-postgres-initdb ConfigMap in installation.toml and VEOVEO_KEYCLOAK_DATABASE_PASSWORD
-- (Secret keycloak-db, key password) to PostgreSQL's environment in postgres.yaml, before
-- PostgreSQL's first start. On an existing one, run it once as the owner, with
-- VEOVEO_KEYCLOAK_DATABASE_PASSWORD in psql's environment.
\set ON_ERROR_STOP on
\getenv keycloak_password VEOVEO_KEYCLOAK_DATABASE_PASSWORD
CREATE ROLE keycloak LOGIN PASSWORD :'keycloak_password';
CREATE DATABASE keycloak OWNER keycloak;
