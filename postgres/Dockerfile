FROM postgres:18.6-trixie@sha256:74935e72241653ca55e0414067e6d8763aceb8a810eb51b452253ec3dcfc4336

# PGDG trixie publishes these exact versions for both amd64 and arm64. Keep the
# upstream PostgreSQL server/client fixed while adding extension dependencies.
RUN apt-mark hold postgresql-18 postgresql-client-18 \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        postgresql-18-pgvector=0.8.7-1.pgdg13+1 \
        postgresql-18-postgis-3=3.6.4+dfsg-2.pgdg13+1 \
        postgresql-18-postgis-3-scripts=3.6.4+dfsg-2.pgdg13+1 \
    && test "$(dpkg-query -W -f='${Version}' postgresql-18)" = "$PG_VERSION" \
    && test "$(dpkg-query -W -f='${Version}' postgresql-client-18)" = "$PG_VERSION" \
    && rm -rf /var/lib/apt/lists/*
