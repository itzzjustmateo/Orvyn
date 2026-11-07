"use client";
import { useEffect, useState } from "react";
import { FolderOpen, Upload, Link, X } from "lucide-react";
import { Input } from "@/components/motion/input";
import { Button } from "@/components/motion/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/motion/tabs";
import { AgentProgress } from "@/components/agents/loading-states/agent-progress";
import type { DownloadState } from "./model-library";

declare global { interface Window { ipc?: { postMessage:(message:string)=>void } } }

export function CustomModelForm({selection,download,onRefresh,onBack}: {
  selection:Record<string,string>;download:DownloadState;onRefresh:()=>Promise<void>;onBack:()=>void;
}) {
  const [mode,setMode]=useState("local");const [name,setName]=useState("");const [category,setCategory]=useState("chat");
  const [url,setUrl]=useState("");const [sha,setSha]=useState("");const [system,setSystem]=useState(true);
  const [submitting,setSubmitting]=useState(false);const [error,setError]=useState("");const [jobId,setJobId]=useState<string|null>(null);
  const busy=["downloading","verifying","importing"].includes(download.status);
  const ownJob=jobId!==null&&download.id===jobId;
  useEffect(()=>{if(!name&&selection.model)setName(selection.model.replace(/\.gguf$/i,"").slice(0,80));},[selection.model,name]);
  async function api(path:string,body:object){const response=await fetch(path,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify(body)});const data=await response.json();if(!response.ok)throw new Error(data.error||"Action failed");return data;}
  function pick(slot:string){if(!window.ipc){setError("Open the USB desktop app to choose a local file.");return;}window.ipc.postMessage(JSON.stringify({action:"pick-gguf",slot}));}
  async function submit(event:React.FormEvent){event.preventDefault();setError("");setSubmitting(true);
    try{
      const common={name:name.trim(),category,system_role:system};
      if(mode==="local"){const result=await api('/api/models/import',common);setJobId(result.id);}
      else{
        if(!url.startsWith('https://')||!/^[a-f0-9]{64}$/i.test(sha.trim()))throw new Error("Enter an HTTPS link and a 64-digit hexadecimal SHA-256.");
        const result=await api('/api/models/custom',{...common,url:url.trim(),sha256:sha.trim()});setJobId(result.id);
        await api('/api/models/download',{id:result.id});
      }
      await onRefresh();
    }catch(error){setError(String(error));}finally{setSubmitting(false);}
  }
  return <form onSubmit={submit} className="space-y-5 p-6">
    <div><h3 className="text-lg font-medium">Add custom model</h3><p className="mt-1 text-sm text-gray-400">GGUF files are copied to the USB. Your original file is preserved.</p></div>
    <Tabs value={mode} onValueChange={value=>{setMode(value);if(value==='url'&&category==='vision')setCategory('chat');}}><TabsList><TabsTrigger value="local"><FolderOpen className="size-4"/>Local file</TabsTrigger><TabsTrigger value="url"><Link className="size-4"/>Download link</TabsTrigger></TabsList></Tabs>
    <div className="grid gap-4 sm:grid-cols-2"><Input label="Your model name" value={name} onChange={setName} required maxLength={80} placeholder="For example: My coding assistant"/>
      <label className="space-y-2 text-sm text-gray-300"><span>Category</span><select value={category} onChange={e=>setCategory(e.target.value)} className="block h-10 w-full rounded-xl border border-white/10 bg-[#2f2f2f] px-3"><option value="chat">Chat</option><option value="code">Code</option><option value="reasoning">Reasoning</option>{mode==='local'&&<option value="vision">Vision / OCR (with projector)</option>}</select></label></div>
    {mode==='local'?<div className="space-y-3">
      <div className="flex items-center gap-3 rounded-xl border border-white/10 p-4"><Button type="button" variant="secondary" onClick={()=>pick('model')} disabled={busy}><FolderOpen className="size-4"/>Choose GGUF</Button><span className="min-w-0 flex-1 truncate text-sm text-gray-300">{selection.model||"No model file selected yet"}</span></div>
      {category==='vision'&&<div className="flex items-center gap-3 rounded-xl border border-white/10 p-4"><Button type="button" variant="secondary" onClick={()=>pick('projector')} disabled={busy}>Choose projector</Button><span className="min-w-0 flex-1 truncate text-sm text-gray-300">{selection.projector||"Matching mmproj .gguf file"}</span>{selection.projector&&<Button type="button" variant="ghost" size="icon" aria-label="Clear projector selection" onClick={async()=>{await api('/api/models/clear-selection',{slot:'projector'});await onRefresh();}}><X className="size-4"/></Button>}</div>}
    </div>:<div className="space-y-4"><Input label="Direct HTTPS link to the GGUF file" type="url" value={url} onChange={setUrl} required placeholder="https://huggingface.co/.../resolve/.../model.gguf"/><Input label="Expected publisher SHA-256" value={sha} onChange={setSha} required maxLength={64} placeholder="64 hexadecimal digits"/></div>}
    <label className="flex items-center gap-3 text-sm text-gray-300"><input type="checkbox" checked={system} onChange={e=>setSystem(e.target.checked)} className="accent-teal-500"/>Model supports system messages <span className="text-xs text-gray-500">(disable for models such as Gemma)</span></label>
    <p className="text-xs text-gray-500">Use GGUF models supported by the included llamafile. Vision models need a matching projector. Adding a model does not load it into memory.</p>
    {error&&<p role="alert" className="rounded-xl bg-red-500/10 p-3 text-sm text-red-300">{error}</p>}
    {ownJob&&<div className="space-y-2 rounded-xl bg-white/5 p-4">{busy&&<AgentProgress label={download.status==='importing'?"Copying and verifying":"Preparing model"}/>}<p className="break-words text-sm text-gray-300">{download.message}</p>{(download.total||0)>0&&<progress className="h-2 w-full accent-teal-500" value={download.bytes||0} max={download.total}/>}</div>}
    <div className="flex flex-wrap gap-3"><Button type="submit" disabled={submitting||busy||!name.trim()||(mode==='local'&&(!selection.model||(category==='vision'&&!selection.projector)))}><Upload className="size-4"/>{mode==='local'?"Copy to USB and add":"Add and download"}</Button><Button type="button" variant="ghost" onClick={onBack}>Back to library</Button></div>
  </form>;
}
