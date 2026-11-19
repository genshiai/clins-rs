import { defineConfig } from '@playwright/test';
export default defineConfig({testDir:'tests', testMatch:'browser.spec.ts', timeout:120_000, use:{baseURL:'http://127.0.0.1:8097'}, webServer:{command:'bun tests/browser-server.ts', url:'http://127.0.0.1:8097', reuseExistingServer:false}});
