"use client";
import { useEffect, useRef, useState } from 'react';
import { api, base64, importDocument, readDocument } from '@/lib/workspace';
import { Opex, emptyOpex, type OpexState } from './opex';
import { CodeBlock, DiffView } from './code-content';

type Task={id:string;text:string;done:boolean};
type Card={question:string;answer:string};
export type WorkbenchState={opex?:OpexState;draft:string;before:string;after:string;tasks:Task[];project:{name:string;text:string;selected:boolean}[];prompts:{name:string;text:string}[];profiles:{id:string;name:string;instructions:string}[];profile:string;source:string;translation:string;target:string;meeting:string;notes:string;cards:Card[]};
export const emptyWorkbench:WorkbenchState={draft:'',before:'',after:'',tasks:[],project:[],prompts:[],profiles:[{id:'teacher',name:'Teacher',instructions:'Explain clearly with examples. Ask questions to check understanding.'},{id:'coder',name:'Coding assistant',instructions:'Provide correct, understandable code in fenced code blocks with a language tag.'}],profile:'',source:'',translation:'',target:'English',meeting:'',notes:'',cards:[]};
type Props={state:WorkbenchState;onChange:(s:WorkbenchState)=>void;onClose:()=>void;ask:(prompt:string)=>Promise<string>;context:string;onPrompt:(text:string)=>void;busy:boolean;temporary:boolean;focusTab?:{name:string;id:number};codeInstructions?:string};
const field='w-full rounded-lg border border-white/10 bg-[#252525] p-2 text-sm';
const button='rounded-lg bg-white/10 px-3 py-2 text-xs hover:bg-white/15 disabled:opacity-40';
function arrayResult(value:string):unknown[]{const begin=value.indexOf('['),end=value.lastIndexOf(']');const result=JSON.parse(value.slice(begin,end+1));if(!Array.isArray(result))throw new Error('AI did not return a valid list. Try again.');return result;}
export function Workbench({state:s,onChange,onClose,ask,context,onPrompt,busy,temporary,focusTab,codeInstructions=""}:Props){
  const [tab,setTab]=useState('Writing'),[error,setError]=useState(''),[working,setWorking]=useState(false),[proposal,setProposal]=useState(''),[instruction,setInstruction]=useState('Make this text clearer.'),[name,setName]=useState(''),[template,setTemplate]=useState('');
  const [docs,setDocs]=useState<{id:string;name:string;collection?:string}[]>([]),[docA,setDocA]=useState(''),[docB,setDocB]=useState(''),[folder,setFolder]=useState(''),[collection,setCollection]=useState('General'),[folders,setFolders]=useState<{folder:string;collection:string}[]>([]);
  const [answers,setAnswers]=useState<Record<number,string>>({}),[grades,setGrades]=useState<Record<number,string>>({});
  const latest=useRef(s);latest.current=s;
  const locked=busy||working;const patch=(v:Partial<WorkbenchState>)=>onChange({...s,...v});
  useEffect(()=>{if(focusTab)setTab(focusTab.name);},[focusTab]);
  async function run(job:()=>Promise<void>){setError('');setWorking(true);try{await job();}catch(e){setError(String(e));}finally{setWorking(false);}}
  function tasks(texts:string[]){patch({tasks:[...s.tasks,...texts.filter(Boolean).map(text=>({id:crypto.randomUUID(),text:text.slice(0,2000),done:false}))]});}
  async function getDocs(){setDocs(await api('/api/documents'));const scan=await api('/api/index/scan',{});setFolders(scan.folders||[]);}
  return <aside aria-label="Workspace" className="flex w-[min(580px,95vw)] shrink-0 flex-col border-l border-white/10 bg-[#171717] max-h-screen overflow-auto p-4 gap-3">
    <div className="flex justify-between"><h2 className="font-semibold">Workspace {temporary?'· temporary':''}</h2><button onClick={onClose} aria-label="Close workspace">✕</button></div>
    <div className="flex flex-wrap gap-1">{['Opex','Writing','Diff','Project','Tasks','Translate','Meeting','Library','Knowledge','Learn'].map(t=><button className={button+(tab===t?' ring-1 ring-teal-400':'')} key={t} onClick={()=>{setTab(t);if(t==='Knowledge')run(getDocs);}}>{t}</button>)}</div>
    {error&&<p role="alert" className="text-sm text-amber-200">{error}</p>}
    {working&&<p role="status" className="text-xs text-teal-300">Working…</p>}
    <fieldset disabled={locked} className="contents">
    {tab==='Opex'&&<Opex project={s.project} state={s.opex||emptyOpex} onChange={opex=>onChange({...latest.current,opex})} onProject={project=>{latest.current={...latest.current,project};onChange(latest.current);}} onImport={()=>setTab('Project')} ask={ask} busy={locked} instructions={codeInstructions}/>}
    {tab==='Writing'&&<>
      <label>Your text<textarea aria-label="Writing text" className={field} rows={15} value={s.draft} onChange={e=>patch({draft:e.target.value})}/></label>
      <input aria-label="Writing instruction" className={field} value={instruction} onChange={e=>setInstruction(e.target.value)}/>
      <button disabled={locked||!s.draft.trim()} className={button} onClick={()=>run(async()=>setProposal(await ask(`${instruction}\nReturn only the updated text.\n\n${s.draft.slice(0,8000)}`)))}>Generate AI proposal</button>
      {proposal&&<><textarea aria-label="Writing proposal" className={field} rows={10} readOnly value={proposal}/><button className={button} onClick={()=>{patch({before:s.draft,after:proposal});setTab('Diff');}}>Compare with my text</button><button className={button} onClick={()=>{patch({draft:proposal});setProposal('');}}>Accept proposal</button></>}
    </>}
    {tab==='Diff'&&<><label>Original<textarea aria-label="Original code" className={field} rows={8} value={s.before} onChange={e=>patch({before:e.target.value})}/></label><label>Proposal<textarea aria-label="New code" className={field} rows={8} value={s.after} onChange={e=>patch({after:e.target.value})}/></label><DiffView before={s.before} after={s.after}/><CodeBlock language="javascript" code={s.after}/></>}
    {tab==='Project'&&<>
      <p className="text-xs text-gray-400">Choose a project folder and select up to 8 source files. Only selected excerpts are shared with chat requests. Maximum 100 files / 2 MB of text.</p>
      <input aria-label="Choose project folder" type="file" multiple {...{webkitdirectory:''} as React.InputHTMLAttributes<HTMLInputElement>} onChange={e=>{const files=Array.from(e.target.files||[]);e.target.value='';run(async()=>{const valid=files.filter(f=>!/(^|\/)(node_modules|\.git|dist|target|build-usb|\.env)(\/|$)/.test(f.webkitRelativePath)&&/\.(tsx?|jsx?|py|rs|go|java|cpp|h|c|md|css|html|json|yaml|yml|toml|sql)$/.test(f.name));if(valid.length>100)throw new Error('Choose a smaller project folder (maximum 100 source files).');if(valid.reduce((n,f)=>n+f.size,0)>2_000_000)throw new Error('Project selection must be no larger than 2 MB.');const project=[];for(const file of valid)project.push({name:file.webkitRelativePath||file.name,text:await readDocument(file),selected:false});patch({project});});}}/>
      {s.project.map((f,i)=><label key={f.name} className="text-xs break-all"><input type="checkbox" checked={f.selected} disabled={!f.selected&&s.project.filter(v=>v.selected).length>=8} onChange={e=>patch({project:s.project.map((v,j)=>j===i?{...v,selected:e.target.checked}:v)})}/> {f.name} ({f.text.length} characters)</label>)}
      <button className={button} onClick={()=>patch({project:[]})}>Clear project selection</button>
    </>}
    {tab==='Tasks'&&<>
      <button className={button} disabled={locked||!context} onClick={()=>run(async()=>{const list=arrayResult(await ask(`Extract concrete commitments and tasks from this conversation. Do not invent anything. Return only a JSON array of strings.\n${context.slice(-10000)}`));tasks(list.filter((v):v is string=>typeof v==='string'));})}>Extract tasks from conversation</button>
      <form className="flex gap-2" onSubmit={e=>{e.preventDefault();if(template.trim()){tasks([template]);setTemplate('');}}}><input aria-label="New task" className={field} value={template} onChange={e=>setTemplate(e.target.value)}/><button className={button}>Add</button></form>
      {s.tasks.map(t=><div className="flex gap-2" key={t.id}><input aria-label={`Task completed: ${t.text}`} type="checkbox" checked={t.done} onChange={e=>patch({tasks:s.tasks.map(v=>v.id===t.id?{...v,done:e.target.checked}:v)})}/><span className={t.done?'line-through text-gray-500':'flex-1'}>{t.text}</span><button aria-label={`Delete task: ${t.text}`} onClick={()=>patch({tasks:s.tasks.filter(v=>v.id!==t.id)})}>✕</button></div>)}
    </>}
    {tab==='Translate'&&<><label>Target language<input className={field} value={s.target} onChange={e=>patch({target:e.target.value})}/></label><div className="grid grid-cols-2 gap-2"><label>Original<textarea aria-label="Translation source" className={field} rows={16} value={s.source} onChange={e=>patch({source:e.target.value})}/></label><label>Translation<textarea aria-label="Translation" className={field} rows={16} value={s.translation} onChange={e=>patch({translation:e.target.value})}/></label></div><button className={button} disabled={locked||!s.source.trim()} onClick={()=>run(async()=>patch({translation:await ask(`Translate the following text into ${s.target}. Return only the translation.\n${s.source.slice(0,8000)}`)}))}>Translate</button></>}
    {tab==='Meeting'&&<><textarea aria-label="Meeting transcript" className={field} rows={12} placeholder="Paste the transcript…" value={s.meeting} onChange={e=>patch({meeting:e.target.value})}/><button className={button} disabled={locked||!s.meeting.trim()} onClick={()=>run(async()=>patch({notes:await ask(`Process this meeting transcript under Summary, Decisions, and Action items. Include an owner and deadline only when stated; otherwise use 'not specified'. Do not invent anything.\n${s.meeting.slice(0,10000)}`)}))}>Process notes</button><textarea aria-label="Meeting notes" className={field} rows={12} value={s.notes} onChange={e=>patch({notes:e.target.value})}/><button disabled={locked||!s.notes} className={button} onClick={()=>run(async()=>{const list=arrayResult(await ask(`Return only concrete action items from these notes as a JSON array of strings.\n${s.notes.slice(0,8000)}`));tasks(list.filter((v):v is string=>typeof v==='string'));setTab('Tasks');})}>Create tasks from action items</button></>}
    {tab==='Library'&&<>
      <label>Assistant profile<select aria-label="Assistant profile" className={field} value={s.profile} onChange={e=>patch({profile:e.target.value})}><option value="">Default assistant</option>{s.profiles.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
      <input aria-label="Template name" placeholder="Name" className={field} value={name} onChange={e=>setName(e.target.value)}/><textarea aria-label="Template text" placeholder="Prompt or assistant instructions" className={field} rows={6} value={template} onChange={e=>setTemplate(e.target.value)}/>
      <div className="flex gap-2"><button disabled={!name.trim()||!template.trim()} className={button} onClick={()=>patch({prompts:[...s.prompts,{name,text:template}]})}>Save prompt</button><button disabled={!name.trim()||!template.trim()} className={button} onClick={()=>patch({profiles:[...s.profiles,{id:crypto.randomUUID(),name,instructions:template}]})}>Save profile</button></div>
      {s.prompts.map((p,i)=><div key={i} className="flex gap-2"><button className={button} onClick={()=>onPrompt(p.text)}>{p.name} use</button><button onClick={()=>patch({prompts:s.prompts.filter((_,j)=>j!==i)})}>Remove</button></div>)}
      {s.profiles.map(p=><div key={p.id} className="flex gap-2 text-xs"><span>{p.name}</span><button onClick={()=>{setName(p.name);setTemplate(p.instructions);}}>Edit as new profile</button><button onClick={()=>patch({profiles:s.profiles.filter(v=>v.id!==p.id),profile:s.profile===p.id?'':s.profile})}>Remove</button></div>)}
    </>}
    {tab==='Knowledge'&&<>
      <label>Knowledge collection<input className={field} value={collection} onChange={e=>setCollection(e.target.value)}/></label>
      <label>Automatically index a USB subfolder<input aria-label="Index folder" className={field} placeholder="For example docs or projects/my-app" value={folder} onChange={e=>setFolder(e.target.value)}/></label><button disabled={locked||temporary||!folder} className={button} onClick={()=>run(async()=>{setFolders(await api('/api/index/folder',{folder,collection}));setError('Folder linked. The index updates every 30 seconds while the app is open.');})}>Link folder</button>
      {folders.map(f=><div key={f.folder} className="text-xs break-all">{f.folder} · {f.collection}<button disabled={temporary} className={button} onClick={()=>run(async()=>setFolders(await api('/api/index/folder',{folder:f.folder,remove:true})))}>Unlink</button></div>)}
      <p className="text-xs text-gray-400">Linked folders remain on this USB. Changed supported files are re-read. Existing document copies are preserved.</p>
      <label>Document A<select className={field} value={docA} onChange={e=>setDocA(e.target.value)}><option value="">Choose…</option>{docs.map(d=><option key={d.id} value={d.id}>{d.collection||'General'} · {d.name}</option>)}</select></label>
      <label>Document B<select className={field} value={docB} onChange={e=>setDocB(e.target.value)}><option value="">Choose…</option>{docs.map(d=><option key={d.id} value={d.id}>{d.collection||'General'} · {d.name}</option>)}</select></label>
      <button className={button} disabled={locked||!docA||!docB||docA===docB} onClick={()=>run(async()=>{const [a,b]=await Promise.all([api('/api/documents/get',{id:docA}),api('/api/documents/get',{id:docB})]);patch({before:a.text,after:b.text});setTab('Diff');})}>Show document differences</button>
      <button className={button} disabled={locked||!docA} onClick={()=>run(async()=>{const doc=await api('/api/documents/get',{id:docA});patch({source:doc.text.slice(0,10000)});setTab('Learn');})}>Use document for learning</button>
    </>}
    {tab==='Learn'&&<>
      <textarea aria-label="Study material" placeholder="Paste study material or choose a document under Knowledge" className={field} rows={8} value={s.source} onChange={e=>patch({source:e.target.value})}/>
      <button className={button} disabled={locked||!s.source.trim()} onClick={()=>run(async()=>{const values=arrayResult(await ask(`Create 5 practice questions/flashcards about this study material. Return only a JSON array of objects with question and answer fields, both strings.\n${s.source.slice(0,8000)}`));const cards=values.filter((v):v is Card=>!!v&&typeof v==='object'&&typeof(v as Card).question==='string'&&typeof(v as Card).answer==='string');if(!cards.length)throw new Error('No valid flashcards received');patch({cards});setAnswers({});setGrades({});})}>Create flashcards / practice quiz</button>
      {s.cards.map((c,i)=><section key={`${i}-${c.question}`} className="rounded-lg border border-white/10 p-3 space-y-2"><p>{i+1}. {c.question}</p><details><summary>Flip card / show answer</summary><p>{c.answer}</p></details><textarea aria-label={`Practice answer ${i+1}`} className={field} value={answers[i]||''} onChange={e=>setAnswers({...answers,[i]:e.target.value})}/><button className={button} disabled={locked||!answers[i]} onClick={()=>run(async()=>{const result=await ask(`Assess this practice answer. Say Correct, Partly correct, or Incorrect and explain briefly. Question: ${c.question}\nReference: ${c.answer}\nStudent answer: ${answers[i]}`);setGrades(g=>({...g,[i]:result}));})}>Assess answer</button>{grades[i]&&<p>{grades[i]}</p>}</section>)}
    </>}
    </fieldset>
  </aside>;
}

export function useDocumentIndex(onStatus:(text:string)=>void,enabled=true){
  const status=useRef(onStatus);status.current=onStatus;
  useEffect(()=>{if(!enabled)return;let stopped=false,running=false;async function tick(){if(running||stopped)return;running=true;try{const data=await api('/api/index/scan',{});for(const item of data.files||[]){if(stopped)break;const bytes=Uint8Array.from(atob(item.base64),(c)=>c.charCodeAt(0));try{await importDocument(new File([bytes],item.name),{source:item.source,collection:item.collection,fingerprint:item.fingerprint});}catch(e){status.current(`Index ${item.name}: ${String(e)}`);}}}catch(e){status.current(`Folder index: ${String(e)}`);}finally{running=false;}}const timer=setInterval(tick,30000);const first=setTimeout(tick,10000);return()=>{stopped=true;clearTimeout(first);clearInterval(timer);};},[enabled]);
}
export async function exportText(text:string,extension:'txt'|'md'|'pdf'){
  if(extension!=='pdf')return api('/api/export',{text,extension});
  const {jsPDF}=await import('jspdf');const pdf=new jsPDF();pdf.setFontSize(10);let y=15;for(const line of pdf.splitTextToSize(text,180)){if(y>280){pdf.addPage();y=15;}pdf.text(line,15,y);y+=5;}return api('/api/export',{extension,base64:base64(pdf.output('arraybuffer'))});
}
