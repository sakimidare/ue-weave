#!/usr/bin/env node
// 遍历公开栏目和分页，为单页工具生成按年份组织的验收目录。
import {mkdir, readFile, writeFile, rename} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {resolve, join, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';

const {values: options, positionals} = parseArgs({allowPositionals:true, options:{
  'retry-failed':{type:'boolean'}, 'retry-images':{type:'boolean'}, refresh:{type:'boolean'},
  'allow-private-network':{type:'boolean'}, resolve:{type:'string',multiple:true},
  ids:{type:'string'}, help:{type:'boolean'},
}});
if(options.help) {
  console.log('node scripts/jwc-archive.mjs [DIR] [--retry-failed | --retry-images | --refresh] [--ids ID,ID] [--allow-private-network] [--resolve HOST=IP]');
  process.exit(0);
}

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const output = resolve(positionals[0] || join(project, 'output/jwc-aggressive-2020-20261106'));
const origin = 'https://jwc.seu.edu.cn';
const binary = join(project, 'target/debug/ue-weave');
const networkArgs = options['allow-private-network'] ? ['--allow-private-network'] : [];
const curlArgs = ['--noproxy','*'];
for(const address of options.resolve || []) {
  const match=address.match(/^([^=]+)=(.+)$/);
  if(!match) throw new Error('--resolve 需要 HOST=IP');
  networkArgs.push('--resolve',address);
  const ip=match[2].includes(':')?`[${match[2]}]`:match[2];
  for(const port of [80,443])curlArgs.push('--resolve',`${match[1]}:${port}:${ip}`);
}
const pause = () => new Promise(resolve => setTimeout(resolve, 300));
const escape = s => String(s ?? '').replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
async function run(command, args, timeout = 120000) {
  return new Promise(resolve => {
    const child = spawn(command, args, {cwd:project, stdio:['ignore','pipe','pipe']});
    let stdout='', stderr='';
    child.stdout.on('data', data => stdout += data);
    child.stderr.on('data', data => stderr += data);
    const timer=setTimeout(() => child.kill('SIGKILL'), timeout);
    child.on('error', error => {clearTimeout(timer); resolve({code:-1,stdout,stderr:stderr+error.message});});
    child.on('close', code => {clearTimeout(timer); resolve({code,stdout,stderr});});
  });
}
await mkdir(join(output,'lists'), {recursive:true});
let state;
try { state=JSON.parse(await readFile(join(output,'state.json'),'utf8')); }
catch(error) { if(error.code !== 'ENOENT') throw error; }
state ??= {queue:[`${origin}/`], visited:[], articles:{}, discovery_errors:[], results:{}};
const queued=new Set(state.queue), visited=new Set(state.visited);
let phase='discovery';
async function checkpoint() {
  state.visited=[...visited];
  await writeFile(join(output,'state.tmp'),JSON.stringify(state));
  await rename(join(output,'state.tmp'),join(output,'state.json'));
  const articles=Object.values(state.articles).sort((a,b)=>b.year-a.year || Number(b.id)-Number(a.id));
  const results=Object.values(state.results);
  const summary={generated_at:new Date().toISOString(),profile:'aggressive',since:2020,phase,
    scope:'官网首页及公开链接可达栏目、全部分页；按文章 URL 年份筛选，文章 ID 去重',
    download_images:true,download_attachments:false,lists:visited.size,pending_lists:state.queue.length,
    discovered:articles.length,processed:results.length,success:results.filter(r=>r.ok).length,
    failed:results.filter(r=>!r.ok).length,restricted:results.filter(r=>r.failure_kind==='restricted').length,
    pdf:results.filter(r=>r.pdf).length,discovery_errors:state.discovery_errors,articles:results};
  await writeFile(join(output,'manifest.json'),JSON.stringify(summary,null,2));
  await writeFile(join(output,'urls.txt'),articles.map(a=>a.url).join('\n'));
  const rows=articles.map(a=>{
    const r=state.results[a.id];
    const links=r?.ok ? ['html','md','typ','json'].map(ext=>`<a href="${r.directory}/reconstructed.${ext}">${ext}</a>`).concat(r.pdf?[`<a href="${r.directory}/result.pdf">PDF</a>`]:[]) : [];
    if(r) links.push(`<a href="${r.directory}/run.log">日志</a>`);
    return `<tr><td>${a.year}</td><td><a href="${escape(a.url)}">${escape(r?.title||a.title||a.id)}</a></td><td>${r?(r.ok?'成功':r.failure_kind==='restricted'?'校内访问':escape(r.failure_reason||'失败')):'待处理'}</td><td>${links.join(' · ')}</td><td>${r?.warning_count??''}</td></tr>`;
  }).join('\n');
  await writeFile(join(output,'index.html'),`<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>教务处 2020 年起文章验收</title><style>body{font:16px system-ui;margin:32px}table{border-collapse:collapse;width:100%}td,th{padding:10px;border-bottom:1px solid #ddd;text-align:left}a{color:#1768a6}input{padding:8px;width:32em}</style><h1>教务处 2020 年起文章验收 · 激进模式</h1><p>${phase}；栏目页 ${visited.size}；发现 ${articles.length}；完成 ${results.length}，成功 ${summary.success}，失败 ${summary.failed}。栏目请求失败 ${state.discovery_errors.length}。</p><p>图片下载到本地，附件保留远程链接。标题、日期、作者等显示在文档顶部。<a href="manifest.json">完整记录</a> · <a href="urls.txt">网址列表</a></p><input placeholder="按标题、年份或状态筛选" oninput="document.querySelectorAll('tbody tr').forEach(r=>r.hidden=!r.textContent.includes(this.value))"><table><thead><tr><th>年份</th><th>文章</th><th>状态</th><th>文件</th><th>警告数</th></tr></thead><tbody>${rows}</tbody></table></html>`);
}
function enqueue(url) {
  if(!visited.has(url)&&!queued.has(url)) {queued.add(url);state.queue.push(url);}
}
function discover(html,base) {
  for(const match of html.matchAll(/<a\b[^>]*\bhref\s*=\s*(["'])(.*?)\1[^>]*>([\s\S]*?)<\/a>/gi)) {
    let url;
    try {url=new URL(match[2].replaceAll('&amp;','&'),base);} catch {continue;}
    if(url.hostname!=='jwc.seu.edu.cn') continue;
    url.protocol='https:';url.hash='';url.search='';
    url.pathname=url.pathname.replace(/^\/_t\d+\//,'/');
    const article=url.pathname.match(/^\/(\d{4})\/\d{4}\/c\d+a(\d+)\/page\.htm$/);
    if(article && Number(article[1])>=2020) {
      const [,year,id]=article;
      state.articles[id] ??= {id,year:Number(year),url:url.href,title:match[3].replace(/<[^>]*>/g,'').trim()};
    } else if(/\/list(?:\d+)?\.(?:htm|psp)$/.test(url.pathname)) {
      url.pathname=url.pathname.replace(/\/list(?:1)?\.(?:htm|psp)$/,'/list.htm');
      enqueue(url.href);
    }
  }
  if(/\/list(?:\d+)?\.htm$/.test(base)) {
    const counts=[...html.matchAll(/class=["']all_pages["'][^>]*>\s*(\d+)/g)].map(m=>Number(m[1]));
    for(let i=2;i<=Math.max(1,...counts);i++) enqueue(base.replace(/\/list(?:\d+)?\.htm$/,`/list${i}.htm`));
  }
}
await checkpoint();
while(state.queue.length) {
  const batch=state.queue.splice(0,4);
  const responses=await Promise.all(batch.map(url=>run('curl',[...curlArgs,'--fail','--location','--max-time','25','--retry','2','--silent','--show-error',url])));
  for(let i=0;i<batch.length;i++) {
    const url=batch[i],result=responses[i];visited.add(url);
    const name=String(visited.size).padStart(5,'0');
    await writeFile(join(output,'lists',`${name}.html`),result.stdout);
    if(result.code!==0) state.discovery_errors.push({url,error:result.stderr});
    else discover(result.stdout,url);
  }
  console.log(`栏目 ${visited.size}，待抓 ${state.queue.length}，文章 ${Object.keys(state.articles).length}`);
  await checkpoint();await pause();
}
phase='articles';await checkpoint();
const selected=options.ids ? new Set(options.ids.split(',')) : null;
const articles=Object.values(state.articles).filter(a=>{
  if(selected)return selected.has(a.id);
  const previous=state.results[a.id];
  return !previous || options.refresh || (options['retry-failed']&&!previous.ok)
    || (options['retry-images']&&previous.image_errors?.length);
}).sort((a,b)=>b.year-a.year||Number(b.id)-Number(a.id));
for(let offset=0;offset<articles.length;offset+=3) {
  const records=await Promise.all(articles.slice(offset,offset+3).map(async article=>{
    const directory=`${article.year}/${article.id}`, destination=join(output,directory);
    await mkdir(destination,{recursive:true});
    const result=await run(binary,[article.url,'--output',destination,'--profile','aggressive','--include-metadata','--download-images','--formats','html,markdown,typst,json','--json','--timeout','25','--retries','1',...networkArgs],180000);
    await writeFile(join(destination,'run.log'),`${article.url}\n${result.stderr}\n${result.stdout}`);
    const record={...article,directory,ok:result.code===0,pdf:false};
    let report;
    try { report=JSON.parse(result.stdout); } catch { record.failure_reason=result.stderr||'进程没有返回运行结果'; }
    if(!record.ok && report) {
      record.error_code=report.code;
      record.failure_reason=report.error;
      record.failure_kind=report.code==='E0015'?'restricted':report.code==='E0016'?'empty-source':'error';
    }
    if(record.ok) {
      record.status=report.status;
      const ir=JSON.parse(await readFile(join(destination,'reconstructed.json'),'utf8'));
      record.title=ir.metadata.title||article.title;
      record.warning_count=report.warnings.filter(w=>w.level==='warning').length;
      record.integrity=report.content_integrity;
      record.image_errors=report.image_errors;
      record.resource_integrity=report.resource_integrity;
      const pdf=await run('typst',['compile',join(destination,'reconstructed.typ'),join(destination,'result.pdf')],60000);
      record.pdf=pdf.code===0;
      await writeFile(join(destination,'typst.log'),pdf.stdout+pdf.stderr);
    }
    return record;
  }));
  for(const record of records) state.results[record.id]=record;
  console.log(`文章 ${Object.keys(state.results).length}/${Object.keys(state.articles).length}，本组失败 ${records.filter(r=>!r.ok).length}`);
  await checkpoint();await pause();
}
phase='complete';await checkpoint();
console.log(`完成：${join(output,'index.html')}`);
