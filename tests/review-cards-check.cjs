const { chromium } = require(process.env.MOVINIGHT_PLAYWRIGHT_PATH || 'playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
assert(process.env.MOVINIGHT_QA_DATA_DIR, 'Set MOVINIGHT_QA_DATA_DIR to the isolated preview directory before running mutation checks');
let browser;
(async () => {
  for (let attempt=0; attempt<20; attempt++) {
    try { browser = await chromium.connectOverCDP('http://127.0.0.1:9229'); break; }
    catch (error) { if (attempt===19) throw error; await new Promise(resolve => setTimeout(resolve,500)); }
  }
  const context = browser.contexts()[0];
  const page = context.pages().find(p => p.url().includes('tauri.localhost'));
  const errors = []; page.on('pageerror', e => errors.push(e.message));
  const invoke = (command, args = {}) => page.evaluate(({command,args}) => window.__TAURI__.core.invoke(command,args), {command,args});
  const config = await invoke('get_ai_workspace');
  assert(config.proposals.length >= 4, 'Use the isolated copy of the populated research batch');
  await page.waitForSelector('#main-app.show');
  await page.evaluate(() => { const input = document.getElementById('app-zoom'); input.value=100; input.dispatchEvent(new Event('input')); });
  await page.locator('#white-list-nav').click();
  await page.locator('#proposal-review').evaluate(el => el.open = true);
  await page.waitForSelector('#proposal-list .result-card');
  const first = page.locator('#proposal-list .result-card').first();
  const layout = await first.evaluate(el => ({posterHeight:el.querySelector('img').getBoundingClientRect().height, grid:getComputedStyle(el.parentElement).display, width:el.getBoundingClientRect().width}));
  assert.equal(await first.locator('.result-info').evaluate(el => getComputedStyle(el).textAlign), 'center');
  assert.equal(await first.locator('[data-review]').first().evaluate(el => getComputedStyle(el).justifyContent), 'center');
  assert.equal(layout.grid, 'grid'); assert(layout.posterHeight >= 200); assert(layout.width < 400);
  await first.evaluate(el => el.dataset.persistTest='yes');
  await page.waitForTimeout(4300);
  assert.equal(await first.getAttribute('data-persist-test'), 'yes', 'Polling preserves card focus and DOM');
  await page.screenshot({path:'.qa-output/review-cards-desktop.png'});
  const initial = config.proposals.length;
  const review = async (location, action) => {
    const card = page.locator('#proposal-list .result-card').first();
    const id = await card.getAttribute('data-proposal-id');
    if (location === 'popup') {
      await card.click();
      await page.waitForSelector('#movie-details-content', {state:'visible'});
      assert(await page.locator('#movie-details-reason').innerText());
      assert(await page.locator('#movie-details-overview').innerText());
      assert.equal(await page.locator('.details-left-column').first().evaluate(el => getComputedStyle(el).overflowY), 'visible');
      assert.equal(await page.locator('#movie-details-actions [data-review]').count(), 2);
      assert.equal(await page.locator('#movie-details-actions [data-action="whitelist"]').count(), 0);
      await page.screenshot({path:'.qa-output/review-details-popup.png'});
    }
    const before = (await invoke('get_ai_workspace')).proposals.length;
    const libraryBefore = await invoke('get_white_list_items');
    await page.locator(`${location==='popup' ? '#movie-details-actions' : '#proposal-list'} [data-proposal="${id}"][data-review="${action}"]`).click();
    await page.waitForFunction(n => Number(document.querySelector('#proposal-count').textContent) === n, before - 1);
    assert.equal(await page.locator('#movie-details-modal.active').count(), 0);
    const libraryAfter = await invoke('get_white_list_items');
    assert.equal(libraryAfter.length, libraryBefore.length + (action === 'approve' ? 1 : 0));
    console.log('PASS',location,action);
  };
  await review('popup', 'approve'); await review('popup', 'reject');
  await review('card', 'approve'); await review('card', 'reject');
  assert.equal((await invoke('get_ai_workspace')).proposals.length, initial - 4);
  const cdp = await context.newCDPSession(page);
  for (const width of [800,390]) {
    await cdp.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth>innerWidth), false);
    await page.screenshot({path:`.qa-output/review-cards-${width}.png`});
  }
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  await page.locator('#discover-nav').click(); await page.waitForSelector('#results-grid .result-card');
  const discover = page.locator('#results-grid .result-card').first();
  const icon = discover.locator('[data-action="whitelist"]');
  await discover.hover(); await page.waitForTimeout(300);
  await icon.hover(); await page.waitForTimeout(200);
  const tooltip = await icon.evaluate(el => ({content:getComputedStyle(el,'::after').content,opacity:getComputedStyle(el,'::after').opacity}));
  assert.match(tooltip.content,/waitlist/); assert.equal(tooltip.opacity,'1');
  await page.screenshot({path:'.qa-output/card-action-tooltip.png'});
  await discover.locator('.result-poster').click(); await page.waitForSelector('#movie-details-content',{state:'visible'});
  const detailActions = page.locator('#movie-details-actions');
  assert.equal(await detailActions.locator('[data-action]').count(),2);
  for (const action of ['whitelist','watch']) {
    const btn = detailActions.locator(`[data-action="${action}"]`);
    const label = await btn.innerText(); await btn.click();
    await page.waitForFunction(({action,label}) => document.querySelector(`#movie-details-actions [data-action="${action}"]`).textContent !== label, {action,label});
    await btn.click(); await page.waitForFunction(({action,label}) => document.querySelector(`#movie-details-actions [data-action="${action}"]`).textContent === label, {action,label});
    console.log('PASS popup',action,'toggle and synchronized labels');
  }
  await page.screenshot({path:'.qa-output/discover-details-actions.png'});
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:390,height:844,deviceScaleFactor:1,mobile:false});
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth>innerWidth),false);
  await page.screenshot({path:'.qa-output/details-actions-390.png'});
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  await page.locator('#close-movie-details').click();
  await page.locator('#search-nav').click();
  await cdp.send('Emulation.setDeviceMetricsOverride',{width:1400,height:900,deviceScaleFactor:1,mobile:false});
  await page.evaluate(() => { const input = document.getElementById('app-zoom'); input.value=145; input.dispatchEvent(new Event('input')); });
  assert.equal(await page.evaluate(() => document.documentElement.scrollHeight <= innerHeight),true);
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),true);
  await page.screenshot({path:'.qa-output/search-empty-final.png'});
  await cdp.send('Emulation.clearDeviceMetricsOverride');
  assert.deepEqual(errors,[]);
  console.log('PASS grid, stable polling, popup metadata, four review actions, tooltips, library toggles, narrow layouts, no runtime errors');
  await browser.close();
})().catch(async e => { await browser?.close(); console.error(e.stack);process.exitCode=1; });
