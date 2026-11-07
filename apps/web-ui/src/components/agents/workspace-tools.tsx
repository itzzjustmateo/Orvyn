"use client";
import { useState } from "react";
import { api, gb, importDocument, type Hardware, type Preferences } from "@/lib/workspace";
import { Button } from "@/components/motion/button";

export function WorkspaceTools({preferences,onPreferences,hardware,onInput,onImage,onDocument,disabled}:{preferences:Preferences;onPreferences:(p:Preferences)=>void;hardware:Hardware|null;onInput:(text:string)=>void;onImage:(image:string)=>void;onDocument:(id:string)=>void;disabled:boolean}){
  const [open,setOpen]=useState(false),[message,setMessage]=useState(""),[busy,setBusy]=useState(false);
  const [documents,setDocuments]=useState<{id:string;name:string}[]>([]),[usage,setUsage]=useState<Record<string,number>|null>(null);
  async function run(action:()=>Promise<void>){setBusy(true);setMessage("");try{await action();}catch(e){setMessage(String(e));}finally{setBusy(false);}}
  return <div className="mb-2 space-y-2 text-xs">
    <div className="flex flex-wrap items-center gap-3">
      <Button size="sm" variant="ghost" onClick={()=>{setOpen(!open);if(!open)run(async()=>setDocuments(await api('/api/documents')));}}>Tools & preferences</Button>
      <label><input type="checkbox" checked={preferences.autoModel} onChange={e=>onPreferences({...preferences,autoModel:e.target.checked})}/> Smart model selection</label>
      <select aria-label="Chat mode" disabled={disabled} className="rounded bg-[#303030] p-2" value={preferences.chatMode||"chat"} onChange={e=>onPreferences({...preferences,chatMode:e.target.value as Preferences["chatMode"]})}><option value="chat">Chat</option><option value="plan">Plan</option><option value="code">Code</option></select>
      <select aria-label="Response style" className="rounded bg-[#303030] p-2" value={preferences.style} onChange={e=>onPreferences({...preferences,style:e.target.value})}>{[["normaal","Balanced"],["kort","Concise"],["uitgebreid","Detailed"],["eenvoudig","Simple"],["technisch","Technical"]].map(([value,label])=><option key={value} value={value}>{label}</option>)}</select>
    </div>
    {open&&<div className="max-h-64 overflow-y-auto space-y-3 rounded-xl border border-white/10 p-3">
      <p>{hardware?`${hardware.cpu} · ${hardware.threads} threads · ${gb(hardware.available_bytes)} free of ${gb(hardware.ram_bytes)} RAM`:"Loading hardware information…"}</p>
      <div className="flex flex-wrap gap-3 items-center">
        <label>Language <select className="bg-[#303030] p-2" value={preferences.language} onChange={e=>onPreferences({...preferences,language:e.target.value})}>{[["Nederlands","Dutch"],["English","English"],["Deutsch","German"],["Français","French"]].map(([value,label])=><option key={value} value={value}>{label}</option>)}</select></label>
        <label><input type="checkbox" checked={preferences.documents} onChange={e=>onPreferences({...preferences,documents:e.target.checked})}/> Search imported USB documents</label>
        <label><input type="checkbox" checked={preferences.web} onChange={e=>onPreferences({...preferences,web:e.target.checked})}/> Search online (Wikipedia)</label>
      </div>
      <p className="text-gray-400">PDF, Word, and Excel are parsed locally (up to 2,000 rows per sheet). Scans require an installed vision model for OCR. Online search sends your query to Wikipedia.</p>
      <label className="block">Add PDF / text / files <input disabled={busy||disabled} type="file" multiple accept=".pdf,.docx,.xlsx,.xls,.txt,.md,.csv,.json,.log,.py,.js,.ts,.rs,.html,.css" onChange={e=>{const files=Array.from(e.target.files||[]);e.target.value="";run(async()=>{for(const file of files){setMessage(`Reading: ${file.name} (OCR may take a while)…`);const result=await importDocument(file,{collection:preferences.collection||'General'});onDocument(result.id);}setDocuments(await api('/api/documents'));onPreferences({...preferences,documents:true});setMessage(`${files.length} document(s) saved.`);});}}/></label>
      <label className="block">Knowledge collection <input className="bg-[#303030] p-2" placeholder="All collections" value={preferences.collection||''} onChange={e=>onPreferences({...preferences,collection:e.target.value})}/></label>
      <label><input type="checkbox" checked={!!preferences.semantic} onChange={e=>onPreferences({...preferences,semantic:e.target.checked})}/> Semantic search (AI expands search terms)</label>
      <label className="block">Discuss document <select disabled={disabled} className="bg-[#303030] p-2" defaultValue="" onChange={e=>onDocument(e.target.value)}><option value="">Search all documents</option>{documents.map(d=><option key={d.id} value={d.id}>{d.name}</option>)}</select></label>
      <label className="block">Add image <input disabled={disabled} type="file" accept="image/png,image/jpeg,image/webp" onChange={e=>{const file=e.target.files?.[0];e.target.value="";if(!file)return;run(async()=>{if(file.size>8_000_000)throw new Error("Images must be no larger than 8 MB.");const reader=new FileReader();const image=await new Promise<string>((resolve,reject)=>{reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(new Error("Failed to read image"));reader.readAsDataURL(file);});onImage(image);});}}/></label>
      <div className="flex flex-wrap gap-2">
        <Button size="sm" disabled={busy||disabled} onClick={()=>run(async()=>{setMessage("Listening, up to 15 seconds…");const language=({Nederlands:'nl-NL',English:'en-US',Deutsch:'de-DE','Français':'fr-FR'} as Record<string,string>)[preferences.language]||'en-US';const result=await api('/api/speech',{action:'listen',language});if(!result.text)throw new Error("No speech recognized. Check the microphone and installed Windows speech language.");onInput(result.text);setMessage(`Speech recognized (${result.language}).`);})}>Dictate (offline Windows)</Button>
        <Button size="sm" variant="secondary" disabled={busy} onClick={()=>run(async()=>setUsage(await api('/api/disk-usage')))}>Calculate storage usage</Button>
        <Button size="sm" variant="ghost" onClick={()=>onPreferences({autoModel:false,style:'normaal',language:'English',favorites:[],web:false,documents:false})}>Reset preferences</Button>
      </div>
      {usage&&<div className="flex flex-wrap gap-4">{Object.entries(usage).map(([key,bytes])=><span key={key}>{({models:'Models',conversations:'Conversations',documents:'Documents',backups:'Backups',profile:'WebView profile'} as Record<string,string>)[key]}: {gb(bytes)}</span>)}</div>}
      <p className="text-gray-400">Speech uses the Windows voices and speech recognizer installed on this PC.</p>
    </div>}
    {message&&<p role="status" className="text-amber-200">{message}</p>}
  </div>;
}
