FROM postgres:19.0-trixie@sha256:bb6e70add742cd868fb62cb3b2f67d1dd2bdd2b65a521e778420748753687c60

# PGDG trixie publishes these exact versions for both amd64 and arm64. Keep the
# upstream PostgreSQL server/client fixed while adding extension dependencies.
RUN apt-mark hold postgresql-19 postgresql-client-19 \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        postgresql-19-pgvector=0.8.7-1.pgdg13+2 \
        postgresql-19-postgis-3=3.7.0+dfsg-1.pgdg13+2 \
        postgresql-19-postgis-3-scripts=3.7.0+dfsg-1.pgdg13+2 \
    && test "$(dpkg-query -W -f='${Version}' postgresql-19)" = "$PG_VERSION" \
    && test "$(dpkg-query -W -f='${Version}' postgresql-client-19)" = "$PG_VERSION" \
    && rm -rf /var/lib/apt/lists/*
