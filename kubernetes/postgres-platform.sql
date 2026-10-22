-- Runs once, after owner.sql, when the Kubernetes installation's data directory is
-- created: the gateway's and Lore's logins with what they own, and connection
-- capacity. Everything after this is plain SQL by Hub.
\set ON_ERROR_STOP on
\getenv gateway_password VEOVEO_GATEWAY_DATABASE_PASSWORD
\getenv lore_password VEOVEO_LORE_DATABASE_PASSWORD
CREATE ROLE openshell_gateway LOGIN PASSWORD :'gateway_password';
CREATE DATABASE openshell_gateway OWNER openshell_gateway;
CREATE ROLE server_lore LOGIN PASSWORD :'lore_password';
\connect veoveo
CREATE SCHEMA lore AUTHORIZATION server_lore;
-- Takes effect when the image restarts the server after initialization.
ALTER SYSTEM SET max_connections = 512;
