const {chromium}=require(process.env.MOVINIGHT_PLAYWRIGHT_PATH || 'playwright');
const assert=require('node:assert/strict');
assert(process.env.MOVINIGHT_QA_DATA_DIR,'Use an isolated QA library');
let browser;
(async()=>{
 browser=await chromium.connectOverCDP(process.env.MOVINIGHT_QA_CDP || 'http://127.0.0.1:9230');
 const page=browser.contexts()[0].pages().find(p=>p.url().includes('tauri.localhost'));
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const invoke=(command,args)=>page.evaluate(({command,args})=>window.__TAURI__.core.invoke(command,args),{command,args});
 await page.reload();await page.waitForSelector('#main-app.show');await page.locator('#discover-nav').click();
 for(const type of ['all','movie','tv']) {
  await page.locator(`[data-type="${type}"].toggle-btn`).click();
  await page.waitForFunction(()=>!document.querySelector('#search-btn').disabled);
  for(const sort of ['popularity.desc','vote_average.desc','vote_average.asc']) {
   await page.locator('#sort-by').selectOption(sort);
   await page.locator('#hide-incomplete').check();
   await page.locator('#search-btn').click();
   await page.waitForFunction(()=>!document.querySelector('#search-btn').disabled);
   const count=await page.locator('#results-grid .result-card').count();
   assert(count>0,`${type} ${sort}: valid titles must not all disappear`);
   assert.equal(await page.locator('#results-grid .no-poster').count(),0);
   let response;
   for(let pageNumber=1;pageNumber<=10;pageNumber++) {
    response=await invoke('discover_titles_page',{pageNumber,filters:{content_type:type,sort_by:sort,hide_incomplete:true}});
    if(response.results.length || pageNumber>=response.total_pages) break;
   }
   assert(response.results.length>0);
   assert(response.results.every(i=>i.vote_average>0 && i.vote_average<10));
   const before=count;
   await page.locator('#view-more-btn').click();
   await page.waitForFunction(n=>document.querySelectorAll('#results-grid .result-card').length>n,before);
   const keys=await page.locator('#results-grid .result-card').evaluateAll(cards=>cards.map(c=>`${c.dataset.type}:${c.dataset.id}`));
   assert.equal(new Set(keys).size,keys.length);
   console.log('PASS filter ON',type,sort,'shows',count,'valid cards and paginates');
   await page.locator('#hide-incomplete').uncheck();await page.locator('#search-btn').click();
   await page.waitForFunction(()=>!document.querySelector('#search-btn').disabled);
   assert((await page.locator('#results-grid .result-card').count())>0);
   console.log('PASS filter OFF',type,sort,'restores ordinary results');
  }
 }
 assert.deepEqual(errors,[]);
 await page.locator('[data-type="all"].toggle-btn').click();
 await page.waitForFunction(()=>!document.querySelector('#search-btn').disabled);
 await page.locator('#sort-by').selectOption('vote_average.desc');await page.locator('#hide-incomplete').check();await page.locator('#search-btn').click();
 await page.waitForFunction(()=>!document.querySelector('#search-btn').disabled);
 await page.screenshot({path:'.qa-output/spam-filter-fixed.png'});
 await browser.close();
})().catch(async error=>{console.error(error.stack);await browser?.close();process.exitCode=1;});
