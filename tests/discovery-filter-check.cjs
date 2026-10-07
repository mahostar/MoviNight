const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

// Execute the actual frontend pagination logic with controlled API pages.
function app(fetchPage) {
  const controls = new Map();
  const context = vm.createContext({
    window:{ __TAURI__:{ core:{ invoke:async (name,args) => fetchPage(args.pageNumber) } } },
    document:{ addEventListener(){}, getElementById(id){ if(!controls.has(id)) controls.set(id,{ disabled:false,textContent:'' }); return controls.get(id); } },
    console, setTimeout, clearTimeout, setInterval
  });
  vm.runInContext(fs.readFileSync('dist/main.js','utf8'),context);
  vm.runInContext(`
    resultsGrid={innerHTML:''};
    currentContentType='all';
    currentFilters={hideIncomplete:true,yearFrom:null,yearTo:null,genreIds:[],sortBy:'vote_average.desc',excludeAnimation:false,watchProviders:[],originalLanguage:null,minRating:0};
    showLoading=()=>{}; showError=()=>{};
    displayResults=async()=>{};
  `,context);
  return context;
}
const title=(id,poster='/poster.jpg',rating=8)=>({id,poster_path:poster,vote_average:rating,content_type:'movie'});
(async()=>{
  const pages=[];
  const context=app(page=>{pages.push(page);return {page,total_pages:20,total_results:400,results:page===1?[title(1,null),title(2,'/poster.jpg',0),title(3,'/poster.jpg',10)]:page===2?[title(4)]:page===3?[title(4)]:[title(5)]};});
  await vm.runInContext('searchContent()',context);
  assert.deepEqual(pages,[1,2]);assert.equal(vm.runInContext('currentPage',context),2);
  assert.equal(vm.runInContext('allResults.length',context),1);assert.equal(vm.runInContext('allResults[0].id',context),4);
  await vm.runInContext('searchContent(true)',context);
  assert.deepEqual(pages,[1,2,3,4]);assert.equal(vm.runInContext('currentPage',context),4);
  assert.equal(vm.runInContext('allResults.length',context),2);
  console.log('PASS empty/incomplete pages are skipped and View More continues after the last consumed page');

  const scanned=[];
  const sparse=app(page=>{scanned.push(page);return {page,total_pages:20,total_results:400,results:page<=10?[title(page,null)]:[title(11)]};});
  await vm.runInContext('searchContent()',sparse);
  assert.equal(scanned.length,10);assert.equal(vm.runInContext('currentPage',sparse),10);
  await vm.runInContext('searchContent(true)',sparse);
  assert.equal(scanned.at(-1),11);assert.equal(vm.runInContext('allResults[0].id',sparse),11);
  console.log('PASS sparse searches have a bounded scan and can resume without restarting page one');

  const unfiltered=[];
  const off=app(page=>{unfiltered.push(page);return {page,total_pages:20,total_results:400,results:[title(1,null,0),title(2,'/poster.jpg',10)]};});
  vm.runInContext('currentFilters.hideIncomplete=false',off);
  await vm.runInContext('searchContent()',off);
  assert.equal(unfiltered.length,1);assert.equal(vm.runInContext('allResults.length',off),2);
  console.log('PASS disabling the filter restores incomplete and endpoint-rated titles');
})().catch(error=>{console.error(error.stack);process.exitCode=1;});
