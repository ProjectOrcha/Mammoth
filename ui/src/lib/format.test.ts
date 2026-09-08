import { describe, expect, it } from 'vitest';
import { bibytes, bytes, duration, fileHref } from './format';
import { escapeHtml } from './html';

describe('operator-facing values', () => {
  it('keeps sub-byte estimates in the B unit', () => {
    expect(bytes(0.4)).toBe('0 B');
    expect(bibytes(-0.4)).toBe('-0 B');
    expect(bibytes(134217728)).toBe('128 MiB');
    expect(bytes(NaN)).toBe('—');
  });

  it('carries rounded seconds into the next minute and hour', () => {
    expect(duration(59.9)).toBe('1m');
    expect(duration(119.9)).toBe('2m');
    expect(duration(3599.9)).toBe('1h 0m');
  });

  it('preserves special characters as filenames, not URL syntax', () => {
    const url = new URL(fileHref('/data/sales #1? 50%.csv'), 'http://localhost');
    expect(url.hash).toBe('');
    expect(url.search).toBe('');
    expect(decodeURIComponent(url.pathname)).toBe('/files/data/sales #1? 50%.csv');
    expect(fileHref('/')).toBe('/files');
  });

  it('renders API strings as text in HTML chart tooltips', () => {
    const div = document.createElement('div');
    const name = '<img src=x onerror="alert(1)"> & \'report\'';
    div.innerHTML = `<b>${escapeHtml(name)}</b>`;
    expect(div.querySelector('img')).toBeNull();
    expect(div.textContent).toBe(name);
  });
});
