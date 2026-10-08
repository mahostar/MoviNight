// Real Tauri WebView QA. Native pickers use debug-only paths; release builds ignore them.
const assert=require('node:assert/strict');
const fs=require('node:fs');const path=require('node:path');const crypto=require('node:crypto');const zlib=require('node:zlib');
const dir=path.resolve(process.env.MOVINIGHT_QA_DATA_DIR||'');
assert(dir.includes(`${path.sep}.qa-data${path.sep}`),'Use an isolated QA data directory');
const output=path.resolve('.qa-output/snapshot-133.zip');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=','base64');
const item=(id,type)=>({id,content_type:type,title:`Snapshot test ${type} ${id}`,overview:'Portable progress fixture',poster_path:'/snapshot-test.png',release_date:'2020-01-01',vote_average:8,white_list_date:'2020-02-03'});
const watched={...item(42,'tv'),watched_date:'2020-04-05',watched_seasons:[1,2],total_seasons_known:3,personal_note:'Preserve this field'};delete watched.white_list_date;
const research={id:'source-batch',text:'Original research table',instructions:'Verify all titles',updated_at:'2020-01-01',agent_note:'Saved matching notes'};
const workspace={research,research_archive:[],proposals:[{proposal_id:'pending-match',research_id:research.id,requested_title:'Original requested movie',reason:'Verified ID, year and format',item:item(44,'movie'),created_at:'2020-01-01'}],suggestions:[{suggestion_id:'pending-season',source:'discovery',reason:'Continue season 3',item:watched,created_at:'2020-01-01',status:'pending',recommended_seasons:[3]},{suggestion_id:'approved-record',source:'waitlist',reason:'Previously approved',item:item(43,'movie'),created_at:'2020-01-01',status:'approved',recommended_seasons:[]}]};
const write=(name,data)=>fs.writeFileSync(path.join(dir,name),JSON.stringify(data,null,2));
if(process.argv.includes('--prepare')){
 fs.mkdirSync(dir,{recursive:true});fs.mkdirSync('.qa-output',{recursive:true});
 for(const[name,data]of Object.entries({'watched.json':[watched],'white_list.json':[item(43,'movie')],'ai_workspace.json':workspace,'config.json':{}}))write(name,data);
 fs.mkdirSync(path.join(dir,'snapshot_images'),{recursive:true});
 const file=`${hash(png)}.png`;fs.writeFileSync(path.join(dir,'snapshot_images',file),png);
 write('snapshot_images/index.json',{'/snapshot-test.png':{path:'/snapshot-test.png',file:`thumbnails/${file}`,mime:'image/png'}});
 console.log('Prepared isolated snapshot fixture');process.exit(0);
}
function unzip(filename){
 const bytes=fs.readFileSync(filename);let end=bytes.length-22;while(end>=0&&bytes.readUInt32LE(end)!==0x06054b50)end--;
 assert(end>=0);const count=bytes.readUInt16LE(end+10);let offset=bytes.readUInt32LE(end+16);const files={};
 for(let i=0;i<count;i++){
  assert.equal(bytes.readUInt32LE(offset),0x02014b50);const method=bytes.readUInt16LE(offset+10),size=bytes.readUInt32LE(offset+20),names=bytes.readUInt16LE(offset+28),extra=bytes.readUInt16LE(offset+30),comment=bytes.readUInt16LE(offset+32),local=bytes.readUInt32LE(offset+42);
  const name=bytes.subarray(offset+46,offset+46+names).toString();const start=local+30+bytes.readUInt16LE(local+26)+bytes.readUInt16LE(local+28),compressed=bytes.subarray(start,start+size);
  files[name]=method===8?zlib.inflateRawSync(compressed):compressed;offset+=46+names+extra+comment;
 }
 return files;
}
let browser;
(async()=>{
 const{chromium}=require(process.env.MOVINIGHT_PLAYWRIGHT_PATH||'playwright');browser=await chromium.connectOverCDP(process.env.MOVINIGHT_QA_CDP||'http://127.0.0.1:9233');
 const page=browser.contexts()[0].pages().find(p=>p.url().includes('tauri.localhost'));assert(page);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const invoke=(command,args={})=>page.evaluate(({command,args})=>window.__TAURI__.core.invoke(command,args),{command,args});
 const open=async()=>{await page.locator('#settings-btn').click();await page.locator('[data-settings="transfer"]').click();};
 const choose=async()=>{await page.locator('#snapshot-choose').click();await page.waitForFunction(()=>!document.querySelector('#snapshot-preview').hidden);};
 const imported=async()=>{await page.locator('#snapshot-import').click();await page.waitForFunction(()=>document.querySelector('#snapshot-status').textContent.startsWith('Progress imported.'));};
 await page.reload();await page.waitForSelector('#main-app.show');await open();assert(!await page.locator('#snapshot-include-key').isChecked());
 await page.locator('#app-zoom').evaluate(el=>{el.value=125;el.dispatchEvent(new Event('input'));});
 await invoke('set_offline_limit',{enabled:false,limitGb:2});write('config.json',{api_key:'source-test-key',private_device_field:'exclude'});
 await page.locator('#snapshot-export').click();await page.waitForFunction(()=>document.querySelector('#snapshot-status').textContent.startsWith('Snapshot saved:'));
 let files=unzip(output);assert.deepEqual(JSON.parse(files['data/config.json']),{});assert.equal(JSON.parse(files['manifest.json']).thumbnails.length,1);assert.equal(JSON.parse(files['data/snapshot_preferences.json']).zoom,125);
 assert(Object.keys(files).every(name=>!name.includes('offline/')&&!name.includes('backups/')&&!name.includes('webview')));
 console.log('PASS real Export button preserves progress and thumbnails while excluding cache and key by default');
 await page.locator('#snapshot-include-key').check();await page.locator('#snapshot-export').click();await page.waitForFunction(()=>document.querySelector('#snapshot-status').textContent.startsWith('Snapshot saved:'));
 files=unzip(output);assert.deepEqual(JSON.parse(files['data/config.json']),{api_key:'source-test-key'});console.log('PASS API key export is optional and excludes other connection fields');
 const destinationWatched={...watched,watched_date:'2018-01-02',watched_seasons:[3],total_seasons_known:5};
 write('watched.json',[destinationWatched]);write('white_list.json',[item(42,'tv'),item(888,'movie')]);write('ai_workspace.json',{research:{...research,id:'destination-batch',text:'Destination research'},research_archive:[],proposals:[],suggestions:[]});write('config.json',{api_key:'destination-test-key',personal_setting:42});
 const before=fs.readFileSync(path.join(dir,'watched.json'));await choose();assert.equal(await page.locator('#snapshot-mode').inputValue(),'merge');assert(await page.locator('#snapshot-use-key-label').isVisible());
 await page.locator('#snapshot-cancel').click();assert.deepEqual(fs.readFileSync(path.join(dir,'watched.json')),before);console.log('PASS preview and cancellation leave destination progress unchanged');
 await choose();await page.locator('#snapshot-use-key').uncheck();await imported();
 let library=await invoke('get_watched_items');assert.equal(library[0].watched_date,'2018-01-02');assert.deepEqual(library[0].watched_seasons,[1,2,3]);assert.equal(library[0].personal_note,'Preserve this field');assert.equal(library[0].total_seasons_known,5);
 let waitlist=await invoke('get_white_list_items');assert.equal(waitlist.length,2);assert(!waitlist.some(i=>i.id===42));assert.equal(JSON.parse(fs.readFileSync(path.join(dir,'config.json'))).api_key,'destination-test-key');assert.equal(await page.locator('#app-zoom').inputValue(),'125');
 let ws=await invoke('get_ai_workspace');assert.equal(ws.proposals.length,1);assert.equal(ws.suggestions.length,2);assert.equal(ws.research_archive[0].id,'source-batch');
 await choose();await page.locator('#snapshot-use-key').uncheck();await imported();assert.equal((await invoke('get_ai_workspace')).suggestions.length,2);
 console.log('PASS merge combines seasons, keeps dates/key/custom fields, preserves pending/history/research, and is repeatable');
 await page.locator('#close-settings').click();await page.locator('#research-nav').click();await page.locator('#research-archive').selectOption('source-batch');await page.locator('#research-archive-open').click();await page.waitForFunction(()=>document.querySelector('#research-text').value==='Original research table');
 console.log('PASS imported research batches can be reopened in Reel Research');
 await open();await choose();await page.locator('#snapshot-mode').selectOption('replace');assert(await page.locator('#snapshot-import').isDisabled());await page.locator('#snapshot-confirm-replace').check();await imported();
 assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dir,'watched.json'))),[watched]);assert.equal((await invoke('get_white_list_items')).length,1);assert.equal(JSON.parse(fs.readFileSync(path.join(dir,'config.json'))).api_key,'source-test-key');
 assert(fs.readdirSync(path.join(dir,'backups')).some(name=>name.startsWith('snapshot-import-')));console.log('PASS replace requires confirmation, restores exact progress and included key, and creates recovery backups');
 assert.equal((await invoke('offline_status')).enabled,false);const image=await invoke('cache_image',{path:'/snapshot-test.png',size:'w342',origin:'other'});assert(image.startsWith('data:image/png;base64,'));
 await page.locator('#close-settings').click();await page.locator('#watched-nav').click();await page.locator('#watched-grid img').first().scrollIntoViewIfNeeded();await page.waitForFunction(()=>document.querySelector('#watched-grid img')?.src.startsWith('data:image/png;base64,'));assert(await page.locator('#watched-grid img').first().evaluate(img=>img.naturalWidth>0));console.log('PASS imported thumbnails render with offline caching disabled');
 write('config.json',{});await page.reload();await page.waitForSelector('#main-app.show');assert.equal((await invoke('get_ai_workspace')).proposals.length,1);assert.deepEqual(JSON.parse(fs.readFileSync(path.join(dir,'watched.json'))),[watched]);await open();
 const cdp=await browser.contexts()[0].newCDPSession(page);await page.locator('#app-zoom').evaluate(el=>{el.value=100;el.dispatchEvent(new Event('input'));});
 for(const width of [1400,800,390,320]){
  await cdp.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:false});await page.waitForTimeout(200);
  assert(await page.locator('#settings-transfer-panel').evaluate(el=>el.getBoundingClientRect().right<=innerWidth),'Transfer panel fits viewport');
  await page.screenshot({path:`.qa-output/snapshot-transfer-${width}.png`});
 }
 await cdp.send('Emulation.clearDeviceMetricsOverride');assert.deepEqual(errors,[]);console.log('PASS import survives reload and transfer controls fit desktop/mobile with no JavaScript errors');await browser.close();
})().catch(async error=>{console.error(error.stack);await browser?.close();process.exitCode=1;});
