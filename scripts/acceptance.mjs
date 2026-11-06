#!/usr/bin/env node
// Public single-site acceptance corpus. No page scripts are executed.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const option = (name, fallback) => args.includes(name) ? args[args.indexOf(name) + 1] : fallback;
if (args.includes('--help')) {
  console.log('node scripts/acceptance.mjs [--output DIR] [--max 200] [--pages 20] [--profile conservative|balanced|aggressive] [--include-metadata] [--download-attachments] [--allow-private-network]');
  process.exit(0);
}
const output = resolve(option('--output', join(project, 'acceptance-output')));
const max = Number(option('--max', '200'));
const pageLimit = Number(option('--pages', '20'));
const profile = option('--profile', 'conservative');
if (!['conservative', 'balanced', 'aggressive'].includes(profile)) throw new Error('无效的 --profile');
if (![max, pageLimit].every(n => Number.isInteger(n) && n > 0)) throw new Error('--max / --pages 必须为正整数');
const binary = join(project, 'target/debug/ue-weave');
const seed = 'https://jwc.seu.edu.cn/_t3454/zxdt/list.psp';
const escape = value => String(value ?? '').replace(/[&<>"']/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[ch]));
const errorCode = (stdout, stderr) => {
  try {
    const code = JSON.parse(stdout)?.code;
    if (typeof code === 'string' && /^[EWN]\d{4}$/.test(code)) return code;
  } catch {}
  return stderr.match(/error\[([EWN]\d{4})\]/)?.[1] ?? null;
};
const pause = () => new Promise(done => setTimeout(done, 350));
function run(command, argv, timeout = 90000) {
  return new Promise(done => {
    const child = spawn(command, argv, { cwd: project, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '', stderr = '';
    child.stdout.on('data', data => stdout += data);
    child.stderr.on('data', data => stderr += data);
    const timer = setTimeout(() => child.kill('SIGKILL'), timeout);
    child.on('error', error => { clearTimeout(timer); done({code: -1, stdout, stderr: stderr + error.message}); });
    child.on('close', code => { clearTimeout(timer); done({code, stdout, stderr}); });
  });
}
await mkdir(join(output, 'lists'), { recursive: true });
const build = await run('cargo', ['build', '--quiet'], 180000);
if (build.code !== 0) throw new Error(build.stderr);
const articles = new Map(), visited = new Set(), queue = [seed], discoveryErrors = [];
while (queue.length && visited.size < pageLimit && articles.size < max) {
  const url = queue.shift();
  if (visited.has(url)) continue;
  visited.add(url);
  console.log(`列表 ${visited.size}: ${url}`);
  const result = await run('curl', ['--fail', '--location', '--max-time', '30', '--retry', '1', '--silent', '--show-error', url]);
  await writeFile(join(output, 'lists', `${visited.size}.log`), result.stderr);
  if (result.code !== 0) { discoveryErrors.push({url, error: result.stderr}); continue; }
  await writeFile(join(output, 'lists', `${visited.size}.html`), result.stdout);
  for (const match of result.stdout.matchAll(/<a\b[^>]*\bhref\s*=\s*(["'])(.*?)\1[^>]*>([\s\S]*?)<\/a>/gi)) {
    let link;
    try { link = new URL(match[2].replaceAll('&amp;', '&'), url); } catch { continue; }
    if (link.origin !== new URL(seed).origin) continue;
    link.hash = ''; link.search = '';
    if (/\/\d{4}\/\d{4}\/c\d+a\d+\/page\.htm$/.test(link.pathname)) {
      if (articles.size < max) articles.set(link.href, match[3].replace(/<[^>]*>/g, '').trim());
    } else if (/\/zxdt\/list(?:\d+)?\.(?:htm|psp)$/.test(link.pathname) && !visited.has(link.href) && !queue.includes(link.href)) {
      queue.push(link.href);
    }
  }
  await pause();
}
const results = [];
async function saveIndex() {
  const summary = { generated_at: new Date().toISOString(), discovered: articles.size, processed: results.length, success: results.filter(r => r.ok).length, failed: results.filter(r => !r.ok).length, discovery_errors: discoveryErrors, articles: results };
  await writeFile(join(output, 'manifest.json'), JSON.stringify(summary, null, 2));
  const rows = results.map(r => `<tr><td>${r.index}</td><td><a href="${escape(r.url)}">${escape(r.title)}</a></td><td>${r.ok ? '成功' : '失败'} / ${escape(r.integrity ?? '')}</td><td>${r.ok ? `<a href="${r.directory}/reconstructed.html">HTML</a> · <a href="${r.directory}/reconstructed.md">Markdown</a> · <a href="${r.directory}/reconstructed.typ">Typst</a> · ${r.pdf ? `<a href="${r.directory}/result.pdf">PDF</a> · ` : ''}<a href="${r.directory}/reconstructed.json">IR</a> · ` : ''}<a href="${r.directory}/run.log">日志</a></td><td>${escape(r.warning_count ?? '')}</td></tr>`).join('\n');
  await writeFile(join(output, 'index.html'), `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>教务处文章还原验收</title><style>body{font:16px system-ui;margin:40px;color:#223}table{border-collapse:collapse;width:100%}td,th{padding:12px;border-bottom:1px solid #ddd;text-align:left}a{color:#1768a6}h1{font-size:26px}</style><h1>教务处文章还原验收</h1><p>已处理 ${summary.processed} / ${summary.discovered}；成功 ${summary.success}；失败 ${summary.failed}。处理记录包含推断、格式清理和未知字段。</p><p>图片已下载；附件${args.includes('--download-attachments') ? '已启用下载' : '保留远程链接'}。PDF 编译日志见各目录。</p><table><thead><tr><th>序号</th><th>文章／原网页</th><th>状态／正文检查</th><th>验收文件</th><th>警告数</th></tr></thead><tbody>${rows}</tbody></table></html>`);
}
await writeFile(join(output, 'urls.txt'), [...articles.keys()].join('\n'));
await saveIndex();
for (const [url, title] of articles) {
  const index = results.length + 1;
  const id = url.match(/a(\d+)\/page/)[1];
  const directory = `${String(index).padStart(3, '0')}-${id}`;
  const destination = join(output, directory);
  await mkdir(destination, {recursive: true});
  console.log(`文章 ${index}/${articles.size}: ${title || url}`);
  const command = [url, '--output', destination, '--download-images', '--formats', 'html,markdown,typst,json', '--json', '--profile', profile];
  if (args.includes('--include-metadata')) command.push('--include-metadata');
  if (args.includes('--download-attachments')) command.push('--download-attachments');
  if (args.includes('--allow-private-network')) command.push('--allow-private-network');
  const result = await run(binary, command, 180000);
  const record = { index, url, title, directory, ok: result.code === 0, pdf: false };
  let log = `${url}\n${result.stderr}\n${result.stdout}`;
  if (!record.ok) {
    const code = errorCode(result.stdout, result.stderr);
    if (code) {
      const explain = await run(binary, ['--explain', code]);
      log += `\n--- ue-weave --explain ${code} ---\n${explain.stdout}${explain.stderr}`;
    }
  }
  await writeFile(join(destination, 'run.log'), log);
  if (record.ok) {
    const ir = JSON.parse(await readFile(join(destination, 'reconstructed.json'), 'utf8'));
    record.title = ir.metadata.title || title || id;
    const report = JSON.parse(result.stdout);
    record.integrity = report.content_integrity?.value;
    record.warning_count = report.warnings.filter(w => w.level === 'warning').length;
    const pdf = await run('typst', ['compile', join(destination, 'reconstructed.typ'), join(destination, 'result.pdf')], 60000);
    record.pdf = pdf.code === 0;
    await writeFile(join(destination, 'typst.log'), pdf.stderr + pdf.stdout);
  }
  results.push(record);
  await saveIndex();
  await pause();
}
console.log(`完成：${output}/index.html，成功 ${results.filter(r => r.ok).length}/${results.length}`);
if (!results.length) process.exitCode = 1;
