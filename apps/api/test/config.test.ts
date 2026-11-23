import { afterEach, describe, expect, it, vi } from 'vitest';

async function loadConfig() {
  vi.resetModules();
  return (await import('../src/config')).config;
}

describe('config.host', () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it('escucha en todas las interfaces si HOST no está definido', async () => {
    vi.stubEnv('HOST', '');
    expect((await loadConfig()).host).toBe('0.0.0.0');
  });

  it('respeta HOST para quedar detrás de un reverse proxy', async () => {
    vi.stubEnv('HOST', '127.0.0.1');
    vi.stubEnv('PORT', '8110');
    const config = await loadConfig();
    expect(config.host).toBe('127.0.0.1');
    expect(config.port).toBe(8110);
  });
});
