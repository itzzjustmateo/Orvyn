import { build } from 'esbuild';
import postcss from 'postcss';
import tailwind from '@tailwindcss/postcss';
import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { resolve } from 'node:path';
await mkdir('dist', {recursive:true});
await copyFile('public/brand/orvyn-logo.png','dist/orvyn-logo.png');
await copyFile('node_modules/pdfjs-dist/build/pdf.worker.min.mjs','dist/pdf.worker.min.mjs');
await build({entryPoints:['src/lib/sandbox.worker.ts'],outfile:'dist/sandbox.worker.js',bundle:true,minify:true,platform:'browser',format:'iife'});
await build({entryPoints:{app:'desktop-entry.tsx'},outdir:'dist',chunkNames:'chunks/[name]-[hash]',splitting:true,format:'esm',bundle:true,minify:true,
  platform:'browser',jsx:'automatic',external:['canvg','html2canvas','dompurify'],alias:{'@':resolve('src')},define:{'process.env.NODE_ENV':'"production"'}});
const source=resolve('src/app/globals.css');
const result=await postcss([tailwind({base:process.cwd()})]).process(await readFile(source,'utf8'),{from:source,to:resolve('dist/app.css')});
await writeFile('dist/app.css',result.css);
await writeFile('dist/index.html','<!doctype html><html lang="en" class="dark"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Orvyn · AI Model Hub</title><link rel="icon" href="/orvyn-logo.png"><link rel="stylesheet" href="/app.css"></head><body><div id="root"></div><script type="module" src="/app.js"></script></body></html>');
console.log('Desktop interface bundled from page.tsx and globals.css.');
