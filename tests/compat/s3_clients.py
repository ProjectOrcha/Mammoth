"""Exercise real boto3 and DuckDB clients against an isolated Mammoth process.

Run: uv run --no-project --with boto3 --with duckdb python tests/compat/s3_clients.py
"""
from pathlib import Path
import hashlib
import json
import os
import signal
import socket
import subprocess
import tempfile
import time
from urllib.request import urlopen

import boto3
from botocore import UNSIGNED
from botocore.config import Config
import duckdb

ROOT = Path(__file__).resolve().parents[2]

def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]

with tempfile.TemporaryDirectory(prefix='mammoth-s3-') as temp:
    temp = Path(temp)
    ui, s3 = port(), port()
    log = (temp / 'service.log').open('w+')
    process = subprocess.Popen([
        str(ROOT / os.environ.get('COMPAT_MAMMOTH_BINARY', 'target/debug/mammoth')), '--local-root', str(temp / 'store'),
        'quickstart', '--no-sample', '--ui-listen', f'127.0.0.1:{ui}',
        '--s3-listen', f'127.0.0.1:{s3}',
    ], stdout=log, stderr=log)
    try:
        for attempt in range(100):
            if process.poll() is not None:
                log.seek(0)
                raise RuntimeError(log.read())
            try:
                with urlopen(f'http://127.0.0.1:{ui}/healthz', timeout=1):
                    break
            except OSError:
                time.sleep(0.05)
        else:
            raise RuntimeError('Gateway startup timed out')
        client = boto3.client('s3', endpoint_url=f'http://127.0.0.1:{s3}', region_name='us-east-1',
            config=Config(signature_version=UNSIGNED, s3={'addressing_style': 'path'},
                          request_checksum_calculation='when_required',
                          response_checksum_validation='when_required'))
        client.create_bucket(Bucket='warehouse')
        payload = b'Mammoth real S3 client test\n'
        result = client.put_object(Bucket='warehouse', Key='notes #?% snow.txt', Body=payload)
        assert result['ETag'] == f'"{hashlib.md5(payload).hexdigest()}"', result
        assert client.get_object(Bucket='warehouse', Key='notes #?% snow.txt')['Body'].read() == payload
        assert client.get_object(Bucket='warehouse', Key='notes #?% snow.txt', Range='bytes=1-6')['Body'].read() == payload[1:7]
        assert client.head_object(Bucket='warehouse', Key='notes #?% snow.txt')['ContentLength'] == len(payload)
        listing = client.list_objects_v2(Bucket='warehouse')
        assert listing['Contents'][0]['Key'] == 'notes #?% snow.txt', listing
        assert listing['Contents'][0]['LastModified'].year >= 2026, listing
        db = duckdb.connect()
        parquet = temp / 'numbers.parquet'
        db.execute("COPY (SELECT i AS number FROM range(1000) AS r(i)) TO '" + str(parquet).replace("'", "''") + "' (FORMAT PARQUET)")
        client.put_object(Bucket='warehouse', Key='tables/numbers.parquet', Body=parquet.read_bytes())
        db.execute('INSTALL httpfs')
        db.execute('LOAD httpfs')
        db.execute(f"CREATE SECRET local_mammoth (TYPE S3, ENDPOINT '127.0.0.1:{s3}', URL_STYLE 'path', USE_SSL false)")
        result = db.execute("SELECT count(*), sum(number) FROM read_parquet('s3://warehouse/tables/*.parquet')").fetchone()
        assert result == (1000, 499500), result
        # Keep SSE connected while shutting down: an open dashboard must not hang exit.
        events = urlopen(f'http://127.0.0.1:{ui}/api/v1/events', timeout=5)
        try:
            assert b'block_health' in events.readline()
            process.send_signal(signal.SIGINT)
            assert process.wait(timeout=5) == 0
        finally:
            events.close()
        db.close()
        client.close()
        print(json.dumps({'graceful_shutdown_with_sse': True, 'boto3': boto3.__version__, 'duckdb': duckdb.__version__, 'rows': result[0], 'sum': result[1], 'status': 'passed'}))
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        log.close()
