import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { startStudioFixture } from './studio-fixture.mjs';

const base = process.env.WVST_STUDIO_URL ?? 'http://127.0.0.1:4173';
const fixture = await startStudioFixture();
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, reducedMotion: 'reduce' });
// Make first-visit behavior deterministic, even when a real Bridge is running.
await context.routeWebSocket('ws://127.0.0.1:35876', socket => socket.onMessage(() => socket.close()));
const page = await context.newPage();
const errors = [];
page.on('pageerror', error => errors.push(error.message));
const screenshot = async name => {
  if (!process.env.WVST_STUDIO_SCREENSHOTS) return;
  await page.evaluate(() => { document.activeElement?.blur(); scrollTo(0, 0); });
  await page.screenshot({ path: `${process.env.WVST_STUDIO_SCREENSHOTS}/${name}.png`, fullPage: true });
};
try {
  const response = await page.goto(`${base}/demo`, { waitUntil: 'networkidle' });
  assert.equal(response.status(), 200);
  assert.equal(await page.evaluate(() => crossOriginIsolated), true);
  assert.equal(await page.locator('h1').count(), 1);
  await page.getByRole('button', { name: 'Connect', exact: true }).waitFor();
  await screenshot('studio-offline');

  await page.getByRole('button', { name: 'Try a synth loop' }).click();
  await page.waitForFunction(() => document.querySelectorAll('.player-waveform > i').length === 96);
  assert.equal(await page.locator('audio').evaluate(audio => audio.duration), 8);
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await page.getByRole('button', { name: 'Pause', exact: true }).waitFor();
  await page.waitForFunction(() => document.querySelector('audio').currentTime > 0);
  await page.getByRole('button', { name: 'Pause', exact: true }).click();

  await page.getByText('Advanced connection settings', { exact: true }).click();
  await page.getByLabel('Bridge endpoint').fill(fixture.endpoint);
  await page.getByLabel('Session token').fill('fixture-token');
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await page.getByRole('button', { name: 'Disconnect', exact: true }).waitFor();
  assert.equal(fixture.state.authenticated, 2, 'control and audio sockets both need a hello');
  assert.equal(fixture.state.calls.filter(call => call.method === 'bridge.hello' && call.params.token === 'fixture-token').length, 2);
  assert.equal(await page.locator('.connection-help').getAttribute('open'), null);

  fixture.state.empty = true;
  await page.getByRole('button', { name: 'Rescan', exact: true }).click();
  await page.getByText('No effects discovered yet.', { exact: true }).waitFor();
  fixture.state.empty = false;
  await page.getByRole('button', { name: 'Rescan', exact: true }).click();
  await page.getByText('Your first effect is one click away.', { exact: true }).waitFor();
  fixture.state.failStart = true;
  await page.getByRole('button', { name: 'Mount effect', exact: true }).click();
  await page.getByRole('alert').filter({ hasText: 'Fixture start failure' }).waitFor();
  assert.ok(fixture.state.calls.some(call => call.method === 'instance.destroy'));

  fixture.state.failStart = false;
  await page.getByRole('button', { name: 'Mount effect', exact: true }).click();
  await page.locator('.wvst-slot-list > li').waitFor();
  const parameter = page.locator('.wvst-parameter-grid input').first();
  await parameter.evaluate(input => {
    input.value = '0.7';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new Event('change', { bubbles: true }));
  });
  await page.locator('.wvst-parameter-grid output').first().filter({ hasText: '70%' }).waitFor();
  await page.getByRole('button', { name: 'Mount effect', exact: true }).click();
  await page.waitForFunction(() => document.querySelectorAll('.wvst-slot-list > li').length === 2);
  const slots = page.locator('.wvst-slot-list > li');
  const firstSlot = await slots.first().locator('input').first().inputValue();
  await slots.last().getByRole('button', { name: 'Move up', exact: true }).click();
  assert.equal(await slots.last().locator('input').first().inputValue(), firstSlot);
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('audio').currentTime > .1);
  await screenshot('studio-connected');
  assert.ok(fixture.state.audioFrames > 0, 'the authorized audio socket must exchange real binary frames');
  await slots.first().getByRole('button', { name: 'Bypass', exact: true }).click();
  assert.ok(await slots.first().getAttribute('class').then(value => value.includes('bypassed')));
  await slots.first().getByRole('button', { name: 'Enable', exact: true }).click();

  await page.locator('.sd-theme-toggle').click();
  await screenshot('studio-dark');
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  await screenshot('studio-mobile');
  await slots.first().getByRole('button', { name: 'Remove', exact: true }).click();
  await page.waitForFunction(() => document.querySelectorAll('.wvst-slot-list > li').length === 1);
  await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await page.getByRole('button', { name: 'Disconnect', exact: true }).waitFor();
  await page.getByRole('button', { name: 'Play', exact: true }).click();
  await page.getByRole('button', { name: 'Pause', exact: true }).waitFor();

  await page.goto(`${base}/zh/demo`, { waitUntil: 'networkidle' });
  await page.getByRole('button', { name: '试听合成片段' }).click();
  await page.getByRole('button', { name: '播放', exact: true }).click();
  await page.getByRole('button', { name: '暂停', exact: true }).waitFor();
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  await screenshot('studio-zh-mobile');
  assert.deepEqual(errors, []);
  console.log('Studio smoke passed: bilingual/mobile, local sample and waveform, authorized audio, scan/empty states, failed-start cleanup, parameters, order, bypass, remove, reconnect, dark theme; no page errors.');
} finally {
  await browser.close();
  fixture.close();
}
