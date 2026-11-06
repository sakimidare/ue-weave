#!/usr/bin/env node
// 复核失败页面的服务器提示，汇总导出诊断。
import {readFile,writeFile} from 'node:fs/promises';
import {spawn} from 'node:child_process';
import {join,resolve} from 'node:path';
const output=resolve(process.argv[2]);
const state=JSON.parse(await readFile(join(output,'state.json'),'utf8'));
const warnings={}, years={};
const failures=[];
for(const record of Object.values(state.results)) {
  const log=await readFile(join(output,record.directory,'run.log'),'utf8');
  const start=log.indexOf('\n{');
  let report={};
  try { report=JSON.parse(log.slice(start+1)); } catch {}
  const year=years[record.year]??={discovered:0,success:0,failed:0,restricted:0};
  year.discovered++;
  year[record.ok?'success':'failed']++;
  if(record.ok) {
    for(const warning of report.warnings||[]) {
      if(warning.level!=='warning') continue;
      const item=warnings[warning.code]??={occurrences:0,articles:[]};
      item.occurrences++;
      if(!item.articles.includes(record.directory))item.articles.push(record.directory);
    }
    record.image_errors=report.image_errors;
    record.resource_integrity=report.resource_integrity;
  } else {
    record.error_code=report.code;
    record.failure_reason=report.error;
    if(report.code==='E0015') record.failure_kind='restricted';
    if(report.code==='E0016') record.failure_kind='empty-source';
    if(report.code!=='E0007' && record.failure_kind==='restricted') year.restricted++;
    if(report.code==='E0007') failures.push(record);
  }
}
function fetch(url) {
  return new Promise(resolve=>{
    const child=spawn('curl',['--fail','--location','--max-time','25','--retry','1','--silent','--show-error',url]);
    let html='',error='';
    child.stdout.on('data',data=>html+=data);
    child.stderr.on('data',data=>error+=data);
    child.on('close',code=>resolve({html,error,code}));
  });
}
for(let offset=0;offset<failures.length;offset+=4) {
  await Promise.all(failures.slice(offset,offset+4).map(async record=>{
    let response;
    try { response={html:await readFile(join(output,record.directory,'response.html'),'utf8'),code:0}; }
    catch(error) { if(error.code!=='ENOENT')throw error; response=await fetch(record.url); }
    await writeFile(join(output,record.directory,'response.html'),response.html);
    const message=response.html.match(/<div\b[^>]*class=["']wp_error_msg["'][^>]*>([\s\S]*?)<\/div>/i)?.[1].replace(/<[^>]*>/g,'').trim();
    const body=response.html.match(/<article\b[^>]*class=["']detail-body["'][^>]*>([\s\S]*?)<\/article>/i)?.[1];
    if(message) {
      record.failure_reason=message;
      record.failure_kind=/校内|权限|登录/.test(message)?'restricted':'site-message';
      if(record.failure_kind==='restricted')years[record.year].restricted++;
    } else if(response.code!==0) {
      record.failure_kind='network';record.failure_reason=response.error.trim();
    } else if(body!==undefined && !/<(?:img|iframe|video|audio|object|embed)\b/i.test(body) && !body.replace(/<!--[\s\S]*?-->/g,'').replace(/<\/?div\b[^>]*>/gi,'').trim()) {
      record.failure_kind='empty-source';record.failure_reason='原网页正文容器为空';
    } else {
      record.failure_kind='selector';record.failure_reason='页面可访问，未找到正文选择器';
    }
  }));
  if(offset%40===0)console.log(`复核失败页 ${Math.min(offset+4,failures.length)}/${failures.length}`);
  if(!process.argv.includes('--cached')) await new Promise(resolve=>setTimeout(resolve,300));
}
await writeFile(join(output,'state.json'),JSON.stringify(state));
await writeFile(join(output,'quality-summary.json'),JSON.stringify({years,warnings,failures:Object.values(state.results).filter(r=>!r.ok)},null,2));
console.log('复核完成');
