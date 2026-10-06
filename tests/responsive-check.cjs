const { chromium } = require(process.env.MOVINIGHT_PLAYWRIGHT_PATH || 'playwright');
const assert = require('node:assert/strict');
let browser;
(async()=>{
 browser=await chromium.connectOverCDP('http://127.0.0.1:9229');
 const context=browser.contexts()[0];const page=context.pages().find(p=>p.url().includes('tauri.localhost'));
 const cdp=await context.newCDPSession(page);
 for(const [width,height] of [[1024,768],[800,700],[600,800],[390,844]]){
  await cdp.send('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});
  await page.locator('#discover-nav').click();await page.waitForTimeout(350);
  const result=await page.evaluate(()=>({width:innerWidth,overflow:document.documentElement.scrollWidth>innerWidth,filterHeight:document.querySelector('.compact-discover').getBoundingClientRect().height,firstCardY:document.querySelector('#results-grid .result-card')?.getBoundingClientRect().y}));
  console.log(JSON.stringify(result));assert(!result.overflow,`No horizontal overflow at ${width}`);
  await page.screenshot({path:`.qa-output/discover-${width}.png`});
  await page.locator('#genres-dropdown summary').click();
  const dropdown=await page.locator('#genres-dropdown .dropdown-panel').boundingBox();
  console.log('dropdown',width,JSON.stringify(dropdown));assert(dropdown.x>=0&&dropdown.x+dropdown.width<=width,`Dropdown fits at ${width}`);
  await page.locator('#research-nav').click();await page.waitForTimeout(350);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);await page.screenshot({path:`.qa-output/research-${width}.png`});
  await page.locator('#picks-nav').click();assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 }
 await cdp.send('Emulation.clearDeviceMetricsOverride');await page.locator('#discover-nav').click();await page.waitForTimeout(350);
 await browser.close();
})().catch(async e=>{await browser?.close();console.error(e.stack);process.exitCode=1;});
