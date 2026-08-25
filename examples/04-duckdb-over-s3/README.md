# 04 · DuckDB over the S3 gateway

The reason the S3 API is Phase 4 and not Phase 9: every tool in the modern data
ecosystem works with your cluster on day one, with a one-line config change.

```bash
mammoth quickstart
mammoth token create --name duckdb --out /tmp/duckdb.token
```

```python
import duckdb

con = duckdb.connect()
con.sql("INSTALL httpfs; LOAD httpfs;")
con.sql("SET s3_endpoint='localhost:9000'")
con.sql("SET s3_use_ssl=false")
con.sql("SET s3_url_style='path'")
con.sql("SET s3_access_key_id='mammoth'")
con.sql("SET s3_secret_access_key='<contents of /tmp/duckdb.token>'")

con.sql("SELECT count(*) FROM 's3://sample/nyc-taxi.parquet'").show()
con.sql("""
    SELECT passenger_count, avg(trip_distance) AS avg_miles
    FROM 's3://sample/nyc-taxi.parquet'
    GROUP BY 1 ORDER BY 1
""").show()
```

The same endpoint works for Spark, Polars, Trino, ClickHouse, pandas, Iceberg
and Delta Lake. Nothing in this file is Mammoth-specific — that is the point.
