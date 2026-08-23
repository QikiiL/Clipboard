import { describe, it, expect } from 'vitest';
import { buildItemQuery, escapeLike, DEFAULT_ITEM_LIMIT } from '../queryBuilder';

describe('escapeLike', () => {
  it('escapes LIKE wildcards with backslash', () => {
    expect(escapeLike('50%')).toBe('50\\%');
    expect(escapeLike('a_b')).toBe('a\\_b');
    expect(escapeLike('x[y]')).toBe('x\\[y\\]');
    expect(escapeLike('a\\b')).toBe('a\\\\b');
  });

  it('leaves normal text untouched', () => {
    expect(escapeLike('hello world')).toBe('hello world');
  });
});

describe('buildItemQuery', () => {
  it('uses ESCAPE clause for LIKE so escaped wildcards match literally', () => {
    const { sql } = buildItemQuery({ searchQuery: '50%' });
    expect(sql).toContain("ESCAPE '\\'");
  });

  it('wraps escaped search term in % wildcards', () => {
    const { params } = buildItemQuery({ searchQuery: '50%' });
    expect(params[0]).toBe('%50\\%%');
    expect(params[1]).toBe('%50\\%%');
  });

  it('applies group and favorites filters', () => {
    const { sql, params } = buildItemQuery({ groupId: 3, favoritesOnly: true });
    expect(sql).toContain('group_id = ?');
    expect(sql).toContain('is_favorite = 1');
    expect(params).toEqual([3, DEFAULT_ITEM_LIMIT]);
  });

  it('accepts a custom limit', () => {
    const { sql, params } = buildItemQuery({ limit: 1000 });
    expect(sql).toContain('LIMIT ?');
    expect(params).toEqual([1000]);
  });

  it('returns no WHERE clause without filters', () => {
    const { sql } = buildItemQuery({});
    expect(sql).not.toContain('WHERE');
    expect(sql).toContain('ORDER BY last_used_at DESC LIMIT ?');
  });
});
