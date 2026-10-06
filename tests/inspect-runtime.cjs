const { chromium } = require(process.env.MOVINIGHT_PLAYWRIGHT_PATH || 'playwright');
const fs = require('node:fs');
let browser;
(async () => {
  browser = await chromium.connectOverCDP('http://127.0.0.1:9229');
  const pages = browser.contexts().flatMap(c => c.pages());
  console.log(pages.map(p => p.url()));
  const page = pages.find(p => p.url().includes('tauri.localhost') || p.url().includes('tauri://')) || pages[0];
  if (!page) throw new Error('No app WebView found');
  await page.waitForSelector('#main-app.show');
  await page.waitForSelector('#results-grid .result-card', { timeout: 60000 });
  const stats = await page.evaluate(() => ({ title: document.title, cards:document.querySelectorAll('#results-grid .result-card').length, filterBox: document.querySelector('.compact-discover').getBoundingClientRect().toJSON(), firstCard:document.querySelector('#results-grid .result-card').getBoundingClientRect().toJSON(), viewport:{width:innerWidth,height:innerHeight}, overflow:document.documentElement.scrollWidth>innerWidth, notices:[...document.querySelectorAll('.toast')].map(e=>e.textContent) }));
  fs.writeFileSync('.qa-output/initial-layout.json',JSON.stringify(stats,null,2));
  console.log(JSON.stringify(stats,null,2));
  await page.screenshot({path:'.qa-output/discover-desktop.png'});
  await browser.close();
})().catch(async e => { await browser?.close(); console.error(e.message);process.exitCode=1;});
