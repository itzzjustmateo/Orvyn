"use client";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { api, defaultPreferences, gb, type Hardware, type Preferences } from "@/lib/workspace";
import { LogoChoices } from "./logo-choices";
import { ApiProviders } from "./api-providers";
const StartupDiagnostics=lazy(()=>import('./startup-diagnostics'));
const TrainingDashboard=lazy(()=>import('./training-dashboard'));
export function AppSettings({open,onClose,preferences:p,onChange,hardware,onRefresh}:{open:boolean;onClose:()=>void;preferences:Preferences;onChange:(p:Preferences)=>void;hardware:Hardware|null;onRefresh:()=>Promise<void>}){
  const ref=useRef<HTMLDialogElement>(null),[tab,setTab]=useState("General"),[message,setMessage]=useState(""),[busy,setBusy]=useState(false),[usage,setUsage]=useState<Record<string,number>|null>(null);
  useEffect(()=>{if(open&&!ref.current?.open)ref.current?.showModal();else if(!open&&ref.current?.open)ref.current.close();},[open]);
  const patch=(v:Partial<Preferences>)=>onChange({...p,...v});
  const field="w-full rounded-xl border border-white/15 bg-[#303030] p-3 text-sm";
  async function run(job:()=>Promise<void>){setBusy(true);setMessage("");try{await job();}catch(e){setMessage(String(e));}finally{setBusy(false);}}
  return <dialog ref={ref} onCancel={onClose} onClose={onClose} aria-labelledby="settings-title" className="m-auto max-h-[90vh] w-[min(850px,94vw)] overflow-auto rounded-3xl border border-white/10 bg-[#212121] p-0 text-white backdrop:bg-black/70">
    <header className="flex items-center gap-3 border-b border-white/10 p-5"><img src="/orvyn-logo.png" alt="" className="size-10 object-contain"/><div className="flex-1"><h2 id="settings-title" className="text-lg font-semibold">Orvyn · Settings</h2><p className="text-xs text-gray-400">Preferences are saved automatically on the USB.</p></div><button aria-label="Close settings" onClick={onClose}>✕</button></header>
    <nav aria-label="Settings categories" className="flex flex-wrap gap-2 border-b border-white/10 p-4">{["General","Appearance","Logo","Opex","API’s","Storage","Startup","Training"].map(t=><button key={t} aria-pressed={tab===t} className={"rounded-lg px-3 py-2 text-sm "+(tab===t?"bg-teal-300 text-black":"bg-white/5")} onClick={()=>{setTab(t);setMessage("");}}>{t}</button>)}</nav>
    {open&&tab==="API’s"?<ApiProviders backLabel="Back to settings" onBack={()=>setTab("General")} onRefresh={onRefresh}/>:<div className="space-y-5 p-6">
      {open&&tab==="Startup"&&<Suspense fallback={<p>Loading startup diagnostics…</p>}><StartupDiagnostics/></Suspense>}
      {open&&tab==="Training"&&<Suspense fallback={<p>Loading training lab…</p>}><TrainingDashboard/></Suspense>}
      {tab==="General"&&<>
        <label className="block text-sm">Response language<select aria-label="Default response language" className={field} value={p.language} onChange={e=>patch({language:e.target.value})}>{[["Nederlands","Dutch"],["English","English"],["Deutsch","German"],["Français","French"]].map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label>
        <label className="block text-sm">Response style<select aria-label="Default response style" className={field} value={p.style} onChange={e=>patch({style:e.target.value})}>{[["normaal","Balanced"],["kort","Concise"],["uitgebreid","Detailed"],["eenvoudig","Simple"],["technisch","Technical"]].map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={p.autoModel} onChange={e=>patch({autoModel:e.target.checked})}/>Smart selection for local models</label>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={p.enterToSend!==false} onChange={e=>patch({enterToSend:e.target.checked})}/>Enter sends a message</label>
        <p className="text-xs text-gray-400">{p.enterToSend===false?"Enter inserts a new line. Ctrl+Enter or the send button sends.":"Shift+Enter inserts a new line."}</p>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={p.documents} onChange={e=>patch({documents:e.target.checked})}/>Search attached documents</label>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={p.web} onChange={e=>patch({web:e.target.checked})}/>Search Wikipedia online</label>
      </>}
      {tab==="Logo"&&<LogoChoices selected={p.logoProposal} onSelect={logoProposal=>patch({logoProposal})}/>}
      {tab==="Appearance"&&<>
        <label className="block text-sm">Text size<select aria-label="Text size" className={field} value={p.fontSize||16} onChange={e=>patch({fontSize:Number(e.target.value)})}><option value={14}>Compact · 14 px</option><option value={16}>Default · 16 px</option><option value={18}>Large · 18 px</option></select></label>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={!!p.highContrast} onChange={e=>patch({highContrast:e.target.checked})}/>High contrast</label>
        <label className="flex gap-3 text-sm"><input type="checkbox" checked={!!p.reduceMotion} onChange={e=>patch({reduceMotion:e.target.checked})}/>Reduce motion</label>
        <div className="rounded-xl border border-white/15 p-4"><p className="font-semibold">Make Orvyn comfortable to read.</p><p className="mt-2 text-sm text-gray-400">Changes apply immediately.</p></div>
      </>}
      {tab==="Opex"&&<>
        <h3 className="font-semibold">Your coding assistant</h3><p className="text-sm text-gray-400">Opex uses your project files and selected text model. Review proposed changes before saving them.</p>
        <label className="block text-sm">Default coding instructions<textarea aria-label="Opex default instructions" className={field} rows={5} maxLength={500} value={p.codeInstructions||""} onChange={e=>patch({codeInstructions:e.target.value})} placeholder="For example: use TypeScript, preserve the existing style, and explain important decisions."/></label>
        <p className="text-xs text-gray-400">The legacy editor uses working copies. Work can save approved changes directly, with a backup.</p>
      </>}
      {tab==="Storage"&&<>
        <p className="text-sm">{hardware?hardware.cpu+" · "+gb(hardware.available_bytes)+" RAM available":"Loading hardware information…"}</p>
        <div className="flex flex-wrap gap-3"><button disabled={busy} className="rounded-lg bg-white/10 px-4 py-2 text-sm" onClick={()=>run(async()=>setUsage(await api("/api/disk-usage")))}>Calculate storage usage</button><button disabled={busy} className="rounded-lg bg-teal-300 px-4 py-2 text-sm text-black" onClick={()=>run(async()=>{const result=await api("/api/backup",{});setMessage("Backup created: "+result.path);})}>Create backup</button></div>
        <p className="text-xs text-gray-400">Backups contain saved conversations and configuration, including encrypted API keys. Model files and unsaved changes are excluded.</p>
        {usage&&<dl className="space-y-2 text-sm">{Object.entries(usage).map(([k,v])=><div key={k} className="flex justify-between"><dt>{({models:"Models",conversations:"Conversations",documents:"Documents",backups:"Backups",profile:"WebView profile"} as Record<string,string>)[k]||k}</dt><dd>{gb(v)}</dd></div>)}</dl>}
      </>}
      {message&&<p role="status" className="break-words text-sm text-teal-300">{message}</p>}
      <footer className="border-t border-white/10 pt-4"><button className="text-xs text-gray-400" onClick={()=>{onChange({...defaultPreferences,favorites:p.favorites});setMessage("Preferences reset. Conversations, favorites, and API connections were preserved.");}}>Restore defaults</button></footer>
    </div>}
  </dialog>;
}
