-- Runs once, when the image creates the data directory. The installation owner owns the
-- database and creates the roles and data-environment databases of registered servers;
-- it is not a superuser.
\getenv owner_password VEOVEO_DATABASE_PASSWORD
CREATE ROLE veoveo_owner LOGIN CREATEROLE CREATEDB PASSWORD :'owner_password';
CREATE DATABASE veoveo OWNER veoveo_owner;
-- Dropping a data-environment database WITH (FORCE) ends sessions a deleted server
-- left behind, which needs the right to signal other backends.
GRANT pg_signal_backend TO veoveo_owner;
