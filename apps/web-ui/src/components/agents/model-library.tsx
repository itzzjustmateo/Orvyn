"use client";
import { useEffect, useRef, useState } from "react";
import { Download, Globe, Loader2, Search, X, Check, Pause, Plus, Star } from "lucide-react";
import { modelQuality } from "./model-quality";
import { api, scores, recommend, type Hardware } from "@/lib/workspace";
import { ApiProviders } from "./api-providers";
import { CustomModelForm } from "./custom-model-form";
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/motion/card";
import { Tabs, TabsList, TabsTrigger } from "@/components/motion/tabs";
import { AnimatedBadge } from "@/components/motion/animated-badge";
import { Button } from "@/components/motion/button";
import { Input } from "@/components/motion/input";
import { AgentProgress } from "@/components/agents/loading-states/agent-progress";

export type CatalogModel = { id:string; name:string; category:string; description:string; installed:boolean; online:boolean; status:string; bytes:number; custom?:boolean; can_download?:boolean; remote?:boolean; provider?:string; store?:boolean; downloads?:number };
export type DownloadState = { id?:string; status:string; message?:string; bytes?:number; total?:number };
const compact=(count:number)=>count>=1e6?`${(count/1e6).toFixed(1)}M`:count>=1e3?`${(count/1e3).toFixed(1)}k`:String(count);
export function ModelLibrary({open,onClose,models,selected,onSelect,download,onDownload,onPause,selection,onRefresh,favorites,onFavorite,hardware,queue,queuePaused,onQueue}: {
  open:boolean;onClose:()=>void;models:CatalogModel[];selected:string;onSelect:(id:string)=>void;
  download:DownloadState;onDownload:(id:string)=>void;onPause:()=>void;
  selection:Record<string,string>;onRefresh:()=>Promise<void>;
  favorites:string[];onFavorite:(id:string)=>void;hardware:Hardware|null;queue:string[];queuePaused:boolean;onQueue:(body:object)=>void;
}) {
  const ref=useRef<HTMLDialogElement>(null);
  const [category,setCategory]=useState("all");const [query,setQuery]=useState("");
  const [adding,setAdding]=useState(false);const [apiSettings,setApiSettings]=useState(false);
  const [store,setStore]=useState<{busy:boolean;message:string;error:boolean}>({busy:false,message:"",error:false});
  const storeBusy=useRef(false);const storeRan=useRef(false);
  async function storeSearch(quiet=false){
    if(storeBusy.current)return;storeBusy.current=true;setStore({busy:true,message:"",error:false});
    try{
      const result=await api("/api/models/store",{query});
      const added=Number(result.added||0);const known=Number(result.known||0);
      setStore({busy:false,error:false,message:added?`${added} ${added===1?"model":"models"} from Hugging Face added to your library`:known?`${known} ${known===1?"model is":"models are"} already in your library`:"No GGUF models found on Hugging Face"});
      await onRefresh();
    }catch(error){
      setStore({busy:false,error:!quiet,message:quiet?"Online models unavailable; showing the models on this USB.":String(error).replace(/^Error:\s*/,"")});
    }finally{storeBusy.current=false;}
  }
  // The library shelf fills itself from the Hugging Face API the first time it opens.
  useEffect(()=>{if(open&&!storeRan.current){storeRan.current=true;storeSearch(true);}},[open]);
  useEffect(()=>{if(open&&!ref.current?.open)ref.current?.showModal();else if(!open&&ref.current?.open)ref.current.close();},[open]);
  const busy=["downloading","verifying","importing"].includes(download.status);
  // Match word by word so "llama 3.2" also finds "Llama-3.2-…" in the library.
  const terms=query.toLowerCase().split(/\s+/).filter(Boolean);
  const visible=models.filter(m=>(category==="all"||(category==="favorites"?favorites.includes(m.id):category==="remote"?m.remote:category==="installed"?m.installed&&!m.remote:category==="custom"?m.custom:m.category===category))&&terms.every(term=>`${m.name} ${m.description}`.toLowerCase().includes(term))).sort((a,b)=>Number(favorites.includes(b.id))-Number(favorites.includes(a.id)));
  return <dialog ref={ref} onCancel={onClose} onClose={onClose} aria-labelledby="model-library-title"
    className="m-auto w-[min(1050px,94vw)] max-h-[90vh] rounded-3xl border border-white/10 bg-[#212121] p-0 text-white shadow-2xl backdrop:bg-black/70">
    <div className="flex items-start justify-between gap-4 border-b border-white/10 p-6">
      <div><h2 id="model-library-title" className="text-xl font-semibold">AI library <span className="text-gray-500">{models.length}</span></h2>
      <p className="mt-1 text-sm text-gray-400">Use a local model on USB or connect an online API provider.</p>
      <p className="mt-2 text-xs text-gray-400">Stars indicate estimated quality within each category, not measured benchmarks. Speed and language quality vary.</p></div>
      <div className="flex items-center gap-2"><Button variant="secondary" size="sm" onClick={()=>{setApiSettings(true);setAdding(false);}}>API providers</Button><Button variant="secondary" size="sm" onClick={()=>{setAdding(true);setApiSettings(false);}}><Plus className="size-4"/>Custom model</Button><Button variant="ghost" size="icon" onClick={onClose} aria-label="Close library"><X className="size-5"/></Button></div>
    </div>
    {open&&apiSettings?<ApiProviders onBack={()=>setApiSettings(false)} onRefresh={onRefresh}/>:adding?<CustomModelForm selection={selection} download={download} onRefresh={onRefresh} onBack={()=>setAdding(false)}/>:<>
    <div className="flex items-start gap-2 px-6 pt-5">
      <Input value={query} onChange={setQuery} placeholder="Search for an AI model, or find GGUF models on Hugging Face…" aria-label="Search models" leftIcon={<Search className="size-4"/>} className="flex-1" onKeyDown={event=>{if(event.key==="Enter"){event.preventDefault();storeSearch();}}}/>
      <Button variant="secondary" size="sm" className="h-11 shrink-0 px-4" disabled={store.busy} onClick={()=>storeSearch()}>{store.busy?<Loader2 className="size-4 animate-spin"/>:<Globe className="size-4"/>}{store.busy?"Searching…":"Find online"}</Button>
    </div>
    {(store.busy||store.message)&&<p role={store.error?"alert":undefined} className={`px-6 pt-2 text-xs ${store.error?"text-amber-300":"text-gray-400"}`}>{store.busy?"Searching Hugging Face for GGUF models…":store.message}</p>}
    <Tabs value={category} onValueChange={setCategory} className="px-6 pt-4">
      <TabsList aria-label="Model categories">{[["all","All"],["favorites","Favorites"],["installed","On USB"],["remote","Online / API"],["custom","Custom models"],["chat","Chat"],["code","Code"],["reasoning","Reasoning"],["vision","Vision / OCR"]].map(([value,label])=><TabsTrigger key={value} value={value}>{label}</TabsTrigger>)}</TabsList>
    </Tabs>
    {(queue.length>0||queuePaused)&&<div className="mx-6 mt-4 rounded-xl border border-white/10 p-3 text-xs"><p>Download queue {queuePaused?'· paused':''} · remains active while the app is open</p>{queue.map(id=><div key={id} className="mt-2 flex justify-between"><span>{models.find(m=>m.id===id)?.name||id}</span><button onClick={()=>onQueue({action:'remove',id})}>Remove</button></div>)}{queuePaused&&<Button size="sm" onClick={()=>onQueue({action:'resume'})}>Resume queue</Button>}</div>}
    <div className="grid grid-cols-1 gap-4 p-6 sm:grid-cols-2 lg:grid-cols-3">
      {visible.map(model=>{
        const quality=model.custom?undefined:modelQuality[model.id];
        const current=download.id===model.id;const downloading=current&&busy;
        const knownTotal=(download.total||0)>0;
        const percent=knownTotal?Math.min(100,Math.round((download.bytes||0)/download.total!*100)):0;
        return <Card key={model.id} size="compact" selected={selected===model.id} className="bg-[#262626]">
          <CardHeader><div className="mb-2 flex items-center justify-between gap-2"><AnimatedBadge size="sm" status={model.online?"success":model.installed?"neutral":"info"}>{model.remote?"Online / API":model.online?"Loaded":model.installed?"On USB":model.store?"From Hugging Face":model.can_download===false?"File missing":"Available to download"}</AnimatedBadge><span className="text-xs text-gray-400">{model.remote?"API":model.bytes>0?`${(model.bytes/1e9).toFixed(2)} GB${model.store&&model.downloads?` · ${compact(model.downloads)} downloads`:""}`:"Unknown size"}</span></div>
            <div className="flex items-start justify-between gap-2"><CardTitle>{model.name}</CardTitle><button aria-label={`${model.name} ${favorites.includes(model.id)?'remove from favorites':'add to favorites'}`} aria-pressed={favorites.includes(model.id)} onClick={()=>onFavorite(model.id)}><Star className={`size-5 ${favorites.includes(model.id)?'fill-teal-400 text-teal-400':'text-gray-500'}`}/></button></div>
            {quality!==undefined?<div role="img" aria-label={`Indicative quality: ${quality} out of 5 stars within the category`} title="Editorial estimate within the category; not a benchmark score" className="my-2 flex items-center gap-1">
              {[1,2,3,4,5].map(star=><Star key={star} aria-hidden="true" className={`size-4 ${star<=quality?"fill-amber-400 text-amber-400":"text-gray-600"}`}/>)}
              <span aria-hidden="true" className="ml-1 text-xs text-gray-400">{quality}/5 · indicatief</span>
            </div>:<p className="my-2 text-xs text-gray-400">{model.store?"Community model from Hugging Face · not rated yet":"Quality not yet evaluated"}</p>}
            {scores(model)&&<div className="my-2 space-y-1">{Object.entries(scores(model)!).map(([label,value])=><div key={label} className="flex justify-between text-xs"><span className="text-gray-400">{label}</span><span role="img" aria-label={`${label}: ${value} out of 5, indicative`} className="text-amber-300">{'★'.repeat(value)}<span className="text-gray-600">{'☆'.repeat(5-value)}</span></span></div>)}</div>}
            <CardDescription>{model.description}</CardDescription></CardHeader>
          <CardContent className="flex-1 text-xs text-gray-500">{model.remote?"Messages are processed by the API provider.":model.bytes===0?"Size is determined after downloading":model.bytes>4e9?"Larger model · more memory and processing time":"Compact model · suitable for local use"}
            <p className="mt-2 text-gray-400">{model.remote?"No local model download required. Your provider API rates apply.":recommend(model,hardware)}</p>
            {downloading&&<div className="mt-3 space-y-2"><AgentProgress label={download.status==="verifying"?"Checking SHA-256":"Downloading"} className="text-xs"/><div role="progressbar" aria-label="Download progress" aria-valuenow={knownTotal?percent:undefined} aria-valuemin={0} aria-valuemax={100} className="h-1.5 overflow-hidden rounded-full bg-white/10"><div className="h-full bg-teal-500 transition-all" style={{width:knownTotal?`${percent}%`:"30%"}}/></div><span>{knownTotal?`${percent}% · `:""}{((download.bytes||0)/1e9).toFixed(2)} GB</span></div>}
            {current&&["error","paused"].includes(download.status)&&<p role={download.status==="error"?"alert":undefined} className="mt-3 break-words text-amber-300">{download.message}</p>}
          </CardContent>
          <CardFooter>{model.installed?<Button size="sm" variant={selected===model.id?"secondary":"primary"} className="w-full" onClick={()=>{onSelect(model.id);onClose();}}><Check className="size-3.5"/>{selected===model.id?"Selected":"Use"}</Button>:
            downloading?<Button size="sm" variant="secondary" className="w-full" onClick={onPause} disabled={download.status==="verifying"}><Pause className="size-3.5"/>Pause</Button>:
            <Button size="sm" className="w-full" disabled={queue.includes(model.id)||model.can_download===false} onClick={()=>onDownload(model.id)}><Download className="size-3.5"/>{model.can_download===false?"Import again":queue.includes(model.id)?"Queued":current&&["paused","error"].includes(download.status)?((download.bytes||0)>0?"Resume":"Try again"):busy?"Add to queue":"Download to USB"}</Button>}
          </CardFooter>
        </Card>;
      })}
      {!visible.length&&<p className="col-span-full py-10 text-center text-gray-400">No models found in this category.</p>}
    </div>
    </>}
    <p className="border-t border-white/10 px-6 py-4 text-xs text-gray-500">Local models: one loaded at a time. Idle models stop automatically. Size indicates download size, not total memory use.</p>
  </dialog>;
}
