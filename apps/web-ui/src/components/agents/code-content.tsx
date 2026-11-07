"use client";
import { useEffect, useRef, useState } from 'react';
import { diffLines } from 'diff';

const highlighter=()=>import("./syntax-highlighter").then(module=>module.highlighter);
const aliases:Record<string,string>={js:'javascript',ts:'typescript',py:'python',rs:'rust',sh:'bash'};
export function CodeBlock({code,language='text',onCompare}:{code:string;language?:string;onCompare?:(text:string)=>void}){
  const [tokens,setTokens]=useState<{content:string;color?:string}[][]|null>(null),[status,setStatus]=useState(''),[running,setRunning]=useState(false);
  const worker=useRef<Worker|null>(null),timer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const lang=aliases[language.toLowerCase()]||language.toLowerCase();
  useEffect(()=>{let alive=true;setTokens(null);if(code.length<50000)highlighter().then(h=>{if(alive&&h.getLoadedLanguages().includes(lang))setTokens(h.codeToTokens(code,{lang,theme:'github-dark'}).tokens);}).catch(()=>{});return()=>{alive=false;};},[code,lang]);
  function stop(){worker.current?.terminate();worker.current=null;if(timer.current)clearTimeout(timer.current);setRunning(false);}
  useEffect(()=>()=>{worker.current?.terminate();if(timer.current)clearTimeout(timer.current);},[]);
  function run(){stop();setRunning(true);setStatus('Loading code runtime…');try{const w=new Worker('/sandbox.worker.js');worker.current=w;w.onmessage=e=>{if(e.data.ready){if(timer.current)clearTimeout(timer.current);setStatus('Running…');timer.current=setTimeout(()=>{setStatus('Stopped: time limit reached.');stop();},4000);return;}setStatus([e.data.output,e.data.error||e.data.result].filter(Boolean).join('\n'));stop();};w.onerror=()=>{setStatus('Code execution failed.');stop();};timer.current=setTimeout(()=>{setStatus('Code runtime took too long to load. Try again.');stop();},30000);w.postMessage(code);}catch(e){setStatus(String(e));stop();}}
  return <section className="my-3 overflow-hidden rounded-xl border border-white/10 bg-[#0d1117] text-sm">
    <div className="flex flex-wrap items-center gap-3 border-b border-white/10 px-3 py-2 text-xs"><span className="mr-auto">{language}</span><button onClick={()=>navigator.clipboard.writeText(code).then(()=>setStatus('Copied.')).catch(()=>setStatus('Copy failed.'))}>Copy code</button>{onCompare&&<button onClick={()=>onCompare(code)}>Compare change</button>}{lang==='javascript'&&(running?<button onClick={()=>{stop();setStatus('Stopped.');}}>Stop code</button>:<button onClick={run}>Run JavaScript</button>)}</div>
    <pre className="max-h-96 overflow-auto p-3 whitespace-pre"><code>{tokens?tokens.map((line,i)=><span key={i} className="block min-h-5">{line.map((token,j)=><span key={j} style={{color:token.color}}>{token.content}</span>)}</span>):code}</code></pre>
    {status&&<pre role="status" className="max-h-48 overflow-auto border-t border-white/10 p-3 whitespace-pre-wrap">{status}</pre>}
    {lang==='javascript'&&<p className="px-3 pb-2 text-xs text-gray-500">QuickJS · 16 MB · 1.5 seconds of execution time · no network, files, DOM, or imports.</p>}
  </section>;
}
export function CodeContent({text,onCompare}:{text:string;onCompare?:(text:string)=>void}){
  const pieces=text.split(/(^```[^\n]*\n[\s\S]*?^```\s*$)/gm);
  return <>{pieces.map((piece,i)=>{const match=piece.match(/^```([^\n]*)\n([\s\S]*?)^```\s*$/m);return match?<CodeBlock key={i} language={match[1].trim().split(/\s/)[0]||'text'} code={match[2].replace(/\n$/,'')} onCompare={onCompare}/>:<span key={i} className="whitespace-pre-wrap">{piece}</span>;})}</>;
}
export function DiffView({before,after}:{before:string;after:string}){
  if(before.length+after.length>100000)return <p>Maximum 100,000 characters for comparison.</p>;
  const parts=diffLines(before,after,{timeout:150,maxEditLength:2000});
  if(!parts)return <p>Too many changes to display. Compare a smaller section.</p>;
  return <pre aria-label="Diff" className="max-h-80 overflow-auto whitespace-pre text-xs">{parts.map((part,i)=><div key={i} className={part.added?'bg-green-950 text-green-200':part.removed?'bg-red-950 text-red-200':'text-gray-400'}>{part.value.split('\n').map((line,j)=><div key={j}>{part.added?'+ ':part.removed?'- ':'  '}{line}</div>)}</div>)}</pre>;
}
