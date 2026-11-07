"use client";
import {useEffect,useState} from 'react';
import {api} from '@/lib/workspace';
type Phase={id:string;label:string;duration_ms:number};
type Report={complete:boolean;total_ms:number;phases:Phase[];note:string;frontend?:{storage_ms?:number;dom_ms?:number;resources?:{path:string;duration_ms:number}[]}};
const seconds=(ms:number)=>`${(ms/1000).toLocaleString('en-GB',{maximumFractionDigits:2})} s`;
export default function StartupDiagnostics(){
 const [report,setReport]=useState<Report|null>(null),[error,setError]=useState('');
 useEffect(()=>{let live=true,pending=false;async function refresh(){if(pending)return;pending=true;try{const value=await api('/api/diagnostics/startup');if(live){setReport(value);setError('');}}catch(e){if(live)setError(String(e));}finally{pending=false;}}void refresh();const timer=setInterval(refresh,5000);return()=>{live=false;clearInterval(timer);};},[]);
 const slowest=report?.phases.reduce<Phase|null>((a,b)=>!a||b.duration_ms>a.duration_ms?b:a,null);
 return <section aria-label="Startup diagnostics" className="space-y-4">
  <div><h3 className="text-lg font-semibold">Where does startup time go?</h3><p className="mt-1 text-sm text-gray-400">Measurements from this app launch. This check does not load a model.</p></div>
  {error&&<p role="alert">{error}</p>}{!report&&!error&&<p role="status">Loading startup measurements…</p>}
  {report&&<>
   <div className="grid gap-3 sm:grid-cols-2"><div className="rounded-xl bg-white/5 p-4"><p className="text-xs text-gray-400">{report.complete?'Interface ready after':'Elapsed so far'}</p><p className="mt-1 text-3xl font-semibold text-teal-300">{seconds(report.total_ms)}</p></div><div className="rounded-xl bg-white/5 p-4"><p className="text-xs text-gray-400">Slowest measured phase</p><p className="mt-2 font-medium">{slowest?.label||'Not measured yet'}</p><p className="text-sm text-gray-400">{slowest&&seconds(slowest.duration_ms)}</p></div></div>
   <ol className="space-y-3">{report.phases.map(p=><li key={p.id}><div className="mb-1 flex justify-between gap-4 text-sm"><span>{p.label}</span><span>{seconds(p.duration_ms)}</span></div><div className="h-2 overflow-hidden rounded-full bg-white/5"><div className="h-full rounded-full bg-teal-300/70" style={{width:`${Math.max(1,100*p.duration_ms/Math.max(1,report.total_ms))}%`}}/></div></li>)}</ol>
   <p className="text-xs text-gray-400">{report.note}</p>
   <details className="rounded-xl border border-white/10 p-3"><summary className="cursor-pointer text-sm">Browser details and slow requests</summary><div className="my-3 space-y-1 text-sm"><p>Loading conversations: {report.frontend?.storage_ms==null?'Not measured yet':seconds(report.frontend.storage_ms)}</p><p>Document loaded: {report.frontend?.dom_ms==null?'Not measured yet':seconds(report.frontend.dom_ms)}</p></div><ul className="space-y-1 text-xs text-gray-400">{report.frontend?.resources?.map((r,i)=><li key={i} className="flex justify-between gap-3"><span className="break-all">{r.path}</span><span className="shrink-0">{seconds(r.duration_ms)}</span></li>)}</ul></details>
  </>}
 </section>;
}
