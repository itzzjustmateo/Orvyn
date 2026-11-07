"use client";
import { useEffect, useState } from "react";
import { Button } from "@/components/motion/button";
import { api } from "@/lib/workspace";

type Provider={id:string;name:string;kind:string;base_url:string;model:string;category:string;key_saved?:boolean};
const presets = [
  { kind: "openai", label: "OpenAI", url: "https://api.openai.com/v1" },
  { kind: "openrouter", label: "OpenRouter", url: "https://openrouter.ai/api/v1" },
  { kind: "groq", label: "Groq", url: "https://api.groq.com/openai/v1" },
  { kind: "deepseek", label: "DeepSeek", url: "https://api.deepseek.com" },
  { kind: "mistral", label: "Mistral AI", url: "https://api.mistral.ai/v1" },
  { kind: "compatible", label: "Other OpenAI-compatible API", url: "" },
];
const empty={id:"",name:"",kind:"openai",base_url:"https://api.openai.com/v1",model:"",category:"chat"};
export function ApiProviders({onBack,onRefresh,backLabel="Back to library"}:{onBack:()=>void;onRefresh:()=>Promise<void>;backLabel?:string}){
  const [entries,setEntries]=useState<Provider[]>([]),[form,setForm]=useState({...empty});
  const [key,setKey]=useState(""),[busy,setBusy]=useState(false),[message,setMessage]=useState(""),[failed,setFailed]=useState(false),[available,setAvailable]=useState<string[]>([]);
  useEffect(()=>{let active=true;api("/api/providers").then(data=>{if(active)setEntries(data);}).catch(e=>{if(active){setFailed(true);setMessage(String(e));}});return()=>{active=false;};},[]);
  const field=(name:keyof typeof empty,value:string)=>setForm(f=>({...f,[name]:value}));
  const payload=()=>({...form,api_key:key});
  async function action(work:()=>Promise<void>){setBusy(true);setMessage("");setFailed(false);try{await work();}catch(e){setFailed(true);setMessage(String(e));}finally{setBusy(false);}}
  function reset(){setForm({...empty});setKey("");setAvailable([]);setMessage("");}
  const style="mt-1 w-full rounded-lg border border-white/15 bg-[#303030] p-2 text-sm";
  return <section className="space-y-4 p-6" aria-label="Configure API providers">
    <div className="flex items-center justify-between"><h3 className="text-lg font-semibold">API providers</h3><Button size="sm" variant="ghost" onClick={onBack}>{backLabel}</Button></div>
    <p className="text-sm text-gray-300">Choose a provider preset or enter a custom OpenAI-compatible endpoint. Messages, context, and selected attachments are sent to your provider. API usage may incur charges.</p>
    <p className="text-xs text-gray-400">Keys are encrypted on the USB for this Windows account. Enter them again on another PC. Smart model selection uses local models only.</p>
    {entries.length>0&&<ul className="space-y-2">{entries.map(p=><li key={p.id} className="flex flex-wrap items-center gap-3 rounded-lg border border-white/10 p-3 text-sm">
      <span className="mr-auto"><strong>{p.name}</strong><span className="block text-xs text-gray-400">{p.kind} · {p.model}</span></span>
      <Button size="sm" variant="secondary" disabled={busy} onClick={()=>{setForm({id:p.id,name:p.name,kind:p.kind,base_url:p.base_url,model:p.model,category:p.category});setKey("");setAvailable([]);setMessage("");}}>Edit</Button>
      <Button size="sm" variant="ghost" disabled={busy} aria-label={p.name+" Remove API connection"} onClick={()=>action(async()=>{await api("/api/providers/remove",{id:p.id});setEntries(await api("/api/providers"));if(form.id===p.id)reset();await onRefresh();setMessage("API connection removed. Conversations were preserved.");})}>Remove</Button>
    </li>)}</ul>}
    <form onSubmit={e=>{e.preventDefault();action(async()=>{const saved=await api("/api/providers",payload());setKey("");setForm(f=>({...f,id:saved.id}));setEntries(await api("/api/providers"));await onRefresh();setMessage("Saved. Select the API model to start chatting.");});}} className="space-y-4">
      <fieldset disabled={busy} className="grid gap-4 sm:grid-cols-2">
        <label className="text-sm">Provider<select aria-label="API provider" className={style} value={form.kind} onChange={e=>{const kind=e.target.value;setForm(f=>({...f,kind,base_url:presets.find(p=>p.kind===kind)?.url??"",model:""}));setKey("");setAvailable([]);}}>{presets.map(p=><option key={p.kind} value={p.kind}>{p.label}</option>)}</select></label>
        <label className="text-sm">Name in the app<input required maxLength={80} aria-label="API model name" className={style} value={form.name} onChange={e=>field("name",e.target.value)} placeholder="My online assistant"/></label>
        <label className="text-sm sm:col-span-2">API base URL<input required aria-label="API base URL" readOnly={form.kind!=="compatible"} className={style} value={form.base_url} onChange={e=>{field("base_url",e.target.value);setAvailable([]);}} placeholder="https://aanbieder.example/v1"/></label>
        <label className="text-sm sm:col-span-2">API key<input type="password" autoComplete="new-password" aria-label="API key" className={style} value={key} onChange={e=>setKey(e.target.value)} placeholder={form.id?"Leave blank to keep the saved key":"Your provider key"}/><span className="text-xs text-gray-400">{form.kind==="compatible"?"Optional if your endpoint does not require a key.":"Use an API key from this provider."}</span></label>
        <label className="text-sm">Model-ID<input required maxLength={200} list="api-model-options" aria-label="API model ID" className={style} value={form.model} onChange={e=>field("model",e.target.value)} placeholder={form.kind==="openrouter"?"aanbieder/modelnaam":"Model ID from the model list"}/><datalist id="api-model-options">{available.map(id=><option key={id} value={id}/>)}</datalist></label>
        <label className="text-sm">Usage<select aria-label="API model category" className={style} value={form.category} onChange={e=>field("category",e.target.value)}><option value="chat">Chat</option><option value="code">Code</option><option value="reasoning">Reasoning</option><option value="vision">Vision (if the model supports it)</option></select></label>
      </fieldset>
      <div className="flex flex-wrap gap-2">
        <Button type="button" variant="secondary" disabled={busy} onClick={()=>action(async()=>{const result=await api("/api/providers/models",payload());setAvailable(result.models);setMessage("Connection successful: "+result.models.length+" model IDs found. Choose a chat model below.");})}>Test connection / load models</Button>
        <Button type="submit" disabled={busy}>{busy?"Working…":"Save API model"}</Button>
        {form.id&&<Button type="button" variant="ghost" disabled={busy} onClick={reset}>New API model</Button>}
      </div>
    </form>
    {message&&<p role={failed?"alert":"status"} className={failed?"text-sm text-amber-300":"text-sm text-teal-300"}>{message}</p>}
  </section>;
}