set -e
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE DATABASE data_service_db;
    CREATE DATABASE scheduling_service_db;
EOSQL