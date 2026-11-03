#!/usr/bin/env node
// Re-export a saved acceptance corpus using its canonical IR and local images.
import { readFileSync, writeFileSync, mkdirSync, cpSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const [sourceArg, outputArg] = process.argv.slice(2);
if (!sourceArg || !outputArg) throw new Error('用法：node scripts/regenerate-acceptance.mjs SOURCE OUTPUT');
const source = resolve(sourceArg), output = resolve(outputArg);
if (existsSync(output)) throw new Error(`输出目录已存在：${output}`);
const original = JSON.parse(readFileSync(join(source, 'manifest.json'), 'utf8'));
const binary = join(project, 'target/debug/ue-weave');
function run(command, args, timeout = 60000) {
  const result = spawnSync(command, args, { cwd: project, encoding: 'utf8', timeout, maxBuffer: 32 * 1024 * 1024 });
  return { ok: result.status === 0, log: (result.stderr || '') + (result.stdout || '') + (result.error?.message || '') };
}
const build = run('cargo', ['build', '--quiet'], 180000);
if (!build.ok) throw new Error(build.log);
mkdirSync(output, { recursive: true });
const results = [];
const escape = value => String(value ?? '').replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]));
function save() {
  const summary = { generated_at: new Date().toISOString(), regenerated_from: source, profile: 'aggressive', download_attachments: false, discovered: original.articles.length, processed: results.length, success: results.filter(r => r.ok).length, failed: results.filter(r => !r.ok).length, articles: results };
  writeFileSync(join(output, 'manifest.json'), JSON.stringify(summary, null, 2));
  const rows = results.map(r => {
    const links = r.ok ? ['html', 'md', 'typ', 'json'].map(ext => `<a href="${escape(r.directory)}/reconstructed.${ext}">${ext}</a>`).concat(r.pdf ? [`<a href="${escape(r.directory)}/result.pdf">PDF</a>`] : []) : [];
    links.push(`<a href="${escape(r.directory)}/run.log">日志</a>`);
    return `<tr><td>${r.index}</td><td><a href="${escape(r.url)}">${escape(r.title)}</a></td><td>${r.ok ? '成功' : '失败'} / ${escape(r.integrity)}</td><td>${links.join(' · ')}</td></tr>`;
  }).join('\n');
  writeFileSync(join(output, 'index.html'), `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>激进模式验收</title><style>body{font:16px system-ui;margin:40px;color:#223}table{border-collapse:collapse;width:100%}td,th{padding:12px;border-bottom:1px solid #ddd;text-align:left}a{color:#1768a6}</style><h1>激进模式验收</h1><p>已处理 ${summary.processed}/${summary.discovered}；成功 ${summary.success}；失败 ${summary.failed}。</p><p>基于已保存的 IR 重新生成；包含段首空白清理和 Markdown 无表头表格支持。显示顶部元数据，沿用本地图片，附件保持远程链接。HTML 为正文片段。正文检查沿用原始提取记录。</p><table><thead><tr><th>序号</th><th>文章</th><th>状态／原始正文检查</th><th>文件</th></tr></thead><tbody>${rows}</tbody></table></html>`);
}
for (const article of original.articles) {
  const destination = join(output, article.directory);
  console.log(`${article.index}/${original.articles.length} ${article.title}`);
  if (!article.ok) {
    cpSync(join(source, article.directory), destination, { recursive: true });
    results.push({ ...article, carried_forward: true });
  } else {
    mkdirSync(destination, { recursive: true });
    const result = run(binary, ['--input-ir', join(source, article.directory, 'reconstructed.json'), '--output', destination, '--formats', 'html,markdown,typst,json', '--profile', 'aggressive', '--include-metadata']);
    writeFileSync(join(destination, 'run.log'), result.log);
    const pdf = result.ok ? run('typst', ['compile', join(destination, 'reconstructed.typ'), join(destination, 'result.pdf')]) : { ok: false, log: '' };
    writeFileSync(join(destination, 'typst.log'), pdf.log);
    results.push({ ...article, ok: result.ok, pdf: pdf.ok });
  }
  save();
}
console.log(`完成：${join(output, 'index.html')}`);
if (results.some((r, i) => original.articles[i].ok && (!r.ok || !r.pdf))) process.exitCode = 1;
