import { chromium, expect } from '../apps/web-ui/node_modules/@playwright/test/index.mjs';
import { createServer } from 'node:http';
import { readFile, mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import JSZip from '../apps/web-ui/node_modules/jszip/lib/index.js';
import * as XLSX from '../apps/web-ui/node_modules/xlsx/xlsx.mjs';

const root=fileURLToPath(new URL('../',import.meta.url));
const server=createServer(async(req,res)=>{
  if(req.url==='/api/chat-stream'){
    for await(const chunk of req){}res.writeHead(200,{'Content-Type':'text/event-stream'});
    res.write('data: {"choices":[{"delta":{"content":"Live eerste deel"}}]}\n\n');
    setTimeout(()=>res.end('data: {"choices":[{"delta":{"content":" en tweede deel"}}]}\n\ndata: [DONE]\n\n'),1500);return;
  }
  const name=req.url==='/'?'index.html':req.url.slice(1);
  if(!['index.html','app.js','app.css','pdf.worker.min.mjs','sandbox.worker.js','orvyn-logo.png'].includes(name)&&!/^chunks\/[a-zA-Z0-9_.-]+\.js$/.test(name)){res.writeHead(404).end();return;}
  try{res.setHeader('Content-Type',name.endsWith('.png')?'image/png':name.endsWith('.css')?'text/css':name.endsWith('.html')?'text/html':'text/javascript');res.end(await readFile(resolve(root,'apps/web-ui/dist',name)));}catch{res.writeHead(500).end();}
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({channel:'msedge',headless:true});
let projectFixtureRoot;
try{
  const page=await browser.newPage({viewport:{width:1400,height:1000}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  const moduleRequests=[];page.on('request',request=>{if(request.url().includes('/chunks/'))moduleRequests.push(request.url());});
  let state={chats:[],activeModel:'qwen3',expandedFolders:['today'],preferences:{},revision:'initial'};
  let queue=[],documents=[],calls=[],exports=[],providers=[],pcActions=[];
  let storageWrites=0;
  const candidate={id:'orvyn1-candidate-test',evaluation:{steps:24,elapsed_seconds:188,baseline_loss:{nl:1.53,code:3.5,web:2.18,pc:3.31},candidate_loss:{nl:1.54,code:2.82,web:1.91,pc:2.03}},review:{decision:'do_not_deploy'},answers:{baseline:[{id:'probe-1',question:'Wat betekent offline?',reference:'Zonder internetverbinding.',answer:'Offline betekent zonder internetverbinding.'}],candidate:[{id:'probe-1',question:'Wat betekent offline?',reference:'Zonder internetverbinding.',answer:'Je hebt geen internetverbinding.'}]}};
  const models={};
  for(const [id,name,category,installed] of [['qwen3','Qwen3','reasoning',true],['coder-3b','Qwen Coder 3B','code',true],['gemma3-4b-vision','Gemma beeld','vision',true],['mimo-7b','MiMo 7B','reasoning',false]])models[id]={name,category,installed,type:category==='vision'?'vision':'chat',bytes:2e9,description:'Testmodel',can_download:true};
  await page.route('**/api/**',async route=>{
    const request=route.request(),path=new URL(request.url()).pathname;
    const body=request.postDataJSON();let data={ok:true};
    if(path==='/api/storage'){
      if(request.method()==='PUT'){storageWrites++;state={...body,revision:String(Date.now())};data={revision:state.revision};}else data=state;
    }else if(path==='/api/index/scan'){expect(request.method()).toBe('POST');expect(body).toEqual({});data={folders:[],files:[]};}
    else if(path==='/api/catalog')data={models,runtime:{active:null,phase:'idle',idle_seconds:1800},download:{status:'idle'},selection:{},queue,queue_paused:false};
    else if(path==='/api/hardware')data={ram_bytes:16e9,available_bytes:10e9,cpu:'Test CPU',threads:8,model_ram_bytes:2e9};
    else if(path==='/api/diagnostics/startup')data={complete:true,total_ms:3100,phases:[{id:'configuration',label:'Reading configuration',duration_ms:100},{id:'interface',label:'Loading interface and conversations',duration_ms:3000}],note:'Browserdetails overlappen met de fasen.',frontend:{storage_ms:120,dom_ms:220,resources:[{path:'/app.js',duration_ms:140}]}};
    else if(path==='/api/training')data={candidates:[candidate],command:'python train_orvyn.py --train',progress:{phase:'completed',step:24,total_steps:24,updated_at:1793380000},dataset:{version:'v2',counts:{train:{nl:28,code:28,web:28,pc:28},eval:{nl:12,code:12,web:12,pc:12}},issues:[]}};
    else if(path==='/api/training/candidate'){expect(body.id).toBe(candidate.id);data=candidate;}
    else if(path==='/api/training/examples')data={examples:[{id:'example-'+body.domain,origin:'Synthetisch voorbeeld',messages:[{role:'system',content:'Provide a geldige actie.'},{role:'user',content:'Lees README.md.'},{role:'assistant',content:'{"action":"read","path":"README.md"}'}]}]};
    else if(path==='/api/chat'||path==='/api/chat-stream'){
      calls.push(body);const prompt=String(body.messages.at(-1).content);if(prompt.includes('LIVE_TEST')){await route.fallback();return;}
      await new Promise(r=>setTimeout(r,100));let content=Array.isArray(body.messages.at(-1).content)?'Wereldoorlog OCR_TEST':`Antwoord van ${body.model}: 1939–1945.`;
      if(prompt.includes('Opex. '))content=JSON.stringify({action:'run',command:'Write-Output OPEX_UI_OK'});
      if(prompt.includes('OPEX_INVALID_TEST'))content=JSON.stringify({action:'run',command:'Write-Output SHOULD_NOT_RUN',approved:true});
      if(prompt.includes('OPEX CODE WORKSPACE'))content=String.fromCharCode(96).repeat(3)+'python\n# OPEX_CHANGED\nprint(42)\n'+String.fromCharCode(96).repeat(3);
      if(prompt.includes('Extract concrete commitments'))content='["Afspraak vanuit het gesprek"]';
      if(prompt.includes('Return only concrete action items'))content='["Jan maakt het verslag dinsdag"]';
      if(prompt.includes('Return only the updated text'))content='Een verbeterde tekst.';
      if(prompt.includes('Translate the following text'))content='Hello there.';
      if(prompt.includes('Process this meeting transcript'))content='Samenvatting: overleg.\nBesluiten: verslag maken.\nActiepunten: Jan maakt het verslag dinsdag.';
      if(prompt.includes('Create 5 practice questions'))content='[{"question":"Wanneer begon de oorlog?","answer":"1939"}]';
      if(prompt.includes('Assess this practice answer'))content='Goed: 1939 is het juiste jaartal.';
      data={choices:[{message:{content}}],usage:{completion_tokens:30}};
    }
    else if(path==='/api/providers'){
      if(request.method()==='POST'){
        expect(body.base_url).toBe('https://openrouter.ai/api/v1');
        const id=body.id||'api-test';const {api_key,...safe}=body;
        if(!body.id)expect(api_key).toBe('ui-test-secret');else expect(api_key).toBe('');
        const provider={...safe,id,key_saved:true};providers=providers.filter(p=>p.id!==id).concat(provider);
        models[id]={name:body.name,category:body.category,type:'chat',installed:true,remote:true,provider:body.kind,description:'OpenRouter API',bytes:0,can_download:false};
        data={id};
      }else data=providers;
    }
    else if(path==='/api/providers/models'){expect(body.api_key).toBe('ui-test-secret');data={models:['openai/test-model','other/test-model']};}
    else if(path==='/api/providers/remove'){providers=providers.filter(p=>p.id!==body.id);delete models[body.id];}
    else if(path==='/api/web-read'){expect(body.url).toBe('https://example.com');data={name:'example.com',url:body.url,text:'WEB_PAGE_MARKER: Example Domain is a documentation example.'};}
    else if(path==='/api/opex/action'){pcActions.push(body);data=body.action==='list'?{files:[]}:{exit_code:0,output:'OPEX_UI_OK'};}
    else if(path==='/api/opex/validate'){if(body.proposal.approved!==undefined){await route.fulfill({status:400,json:{error:'Proposal bevat onverwachte goedkeuring.',valid:false}});return;}data={valid:true,proposal:body.proposal};}
    else if(path==='/api/opex/export'){expect(body.name).toBe('example.py');expect(body.text).toContain('OPEX_CHANGED');data={path:'data/exports/opex-test/example.py'};}
    else if(path==='/api/backup')data={path:'backups/orvyn-test.zip'};
    else if(path==='/api/export'){exports.push(body);data={path:'data/exports/test.pdf'};}
    else if(path==='/api/documents'){
      if(request.method()==='POST'){expect(body.text).toContain('Wereldoorlog');const id=`doc${documents.length+1}`;documents.push({...body,id});data={id};}else data=documents;
    }else if(path==='/api/documents/search')data=[{id:'doc1',name:'historie.pdf',section:1,text:'De Tweede Wereldoorlog was van 1939 tot 1945.'}];
    else if(path==='/api/models/download')queue.push(body.id);
    else if(path==='/api/models/queue')queue=queue.filter(id=>id!==body.id);
    else if(path==='/api/disk-usage')data={models:2e9,conversations:5000,documents:2000,backups:1000,profile:1e6};
    await route.fulfill({json:data});
  });
  await page.route('**/sandbox.worker.js',async route=>{await new Promise(r=>setTimeout(r,4500));await route.continue();},{times:1});
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await expect(page.getByText('Stops after 30 minutes idle.',{exact:false})).toBeVisible();
  await page.waitForTimeout(300);
  expect(storageWrites).toBe(0);
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await expect(page.getByLabel('Default response language')).toHaveValue('English');
  await page.getByRole('button',{name:'Close settings'}).click();
  await page.getByRole('button',{name:'AI library',exact:false}).click();
  await page.getByRole('button',{name:'Qwen Coder 3B add to favorites'}).click();
  await expect(page.getByRole('button',{name:'Qwen Coder 3B remove from favorites'})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByText('Likely fits in free RAM',{exact:false}).first()).toBeVisible({timeout:15000});
  await page.getByRole('button',{name:'Download to USB',exact:true}).click();
  await expect(page.getByRole('button',{name:'Queued',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Close library'}).click();
  expect(moduleRequests.filter(url=>/syntax-highlighter|training-dashboard|startup-diagnostics|\/pdf-|\/xlsx-|\/jspdf|\/lib-/.test(url))).toHaveLength(0);
  await page.screenshot({path:resolve(root,'tests/orvyn-chat-home.png')});
  await page.locator('summary').filter({hasText:/^Chat options$/}).click();
  await page.getByLabel('Smart model selection',{exact:true}).check();
  await page.getByLabel('Compare with').selectOption('qwen3');
  await page.getByRole('textbox',{name:'Chat message'}).fill('Schrijf Python code');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect(page.getByText('Antwoord van coder-3b: 1939–1945.')).toBeVisible();
  await expect(page.getByText('Antwoord van qwen3: 1939–1945.')).toBeVisible();
  expect(calls.map(c=>c.model)).toEqual(['coder-3b','qwen3']);
  await page.locator('summary').filter({hasText:/^Conversation folder$/}).click();
  await page.getByLabel('Folder for this conversation').fill('School');
  await expect.poll(()=>state.chats[0]?.folder).toBe('School');
  await page.getByLabel('Compare with').selectOption('');
  await page.getByLabel('Retry response').click();
  await expect.poll(()=>state.chats.length).toBe(2);
  await expect(page.getByRole('button',{name:'Send message'})).toBeVisible();
  await page.getByRole('button',{name:'Tools & preferences'}).click();
  const pdf=makePdf('De Tweede Wereldoorlog was van 1939 tot 1945.');
  await page.locator('input[type=file][accept^=".pdf"]').setInputFiles({name:'historie.pdf',mimeType:'application/pdf',buffer:pdf});
  await expect(page.getByText('1 document(s) saved.')).toBeVisible({timeout:30000});
  expect(documents[0].text).toContain('Page 1');
  const zip=new JSZip();zip.file('[Content_Types].xml','<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>');zip.file('_rels/.rels','<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>');zip.file('word/document.xml','<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Wereldoorlog DOCX_TEST</w:t></w:r></w:p></w:body></w:document>');
  await page.locator('input[type=file][accept^=".pdf"]').setInputFiles({name:'historie.docx',mimeType:'application/vnd.openxmlformats-officedocument.wordprocessingml.document',buffer:await zip.generateAsync({type:'nodebuffer'})});
  await expect.poll(()=>documents.length,{timeout:30000}).toBe(2);expect(documents[1].text).toContain('DOCX_TEST');
  const workbook=XLSX.utils.book_new();XLSX.utils.book_append_sheet(workbook,XLSX.utils.aoa_to_sheet([['Wereldoorlog','Begin','Einde'],['Tweede',1939,1945]]),'Geschiedenis');
  await expect(page.locator('input[type=file][accept^=".pdf"]')).toBeEnabled();
  await page.locator('input[type=file][accept^=".pdf"]').setInputFiles({name:'historie.xlsx',mimeType:'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',buffer:XLSX.write(workbook,{type:'buffer',bookType:'xlsx'})});
  await expect.poll(()=>documents.length,{timeout:30000}).toBe(3);expect(documents[2].text).toContain('1939');
  await expect(page.locator('input[type=file][accept^=".pdf"]')).toBeEnabled();
  await page.locator('input[type=file][accept^=".pdf"]').setInputFiles({name:'scan.pdf',mimeType:'application/pdf',buffer:makePdf('')});
  await expect.poll(()=>documents.length,{timeout:30000}).toBe(4);expect(documents[3].text).toContain('OCR_TEST');
  await page.getByRole('textbox',{name:'Chat message'}).fill('Wanneer was de Wereldoorlog?');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect(page.getByText('Sources (1)')).toBeVisible();
  expect(calls.at(-1).messages.at(-1).content).toContain('[1] historie.pdf');
  await page.getByLabel('Response style',{exact:true}).selectOption('kort');
  await expect.poll(()=>state.preferences.style).toBe('kort');
  await page.reload();
  await page.locator('summary').filter({hasText:/^Chat options$/}).click();
  await expect(page.getByLabel('Response style',{exact:true})).toHaveValue('kort');
  await page.getByLabel('Search conversations').fill('geen-bestaand-woord');
  await expect(page.getByText('Today',{exact:true})).toHaveCount(0);
  await page.getByLabel('Search conversations').fill('');
  await page.getByRole('button',{name:'Workspace',exact:true}).click();
  await page.getByRole('button',{name:'Project',exact:true}).click();
  // Create the upload fixture from source so clean checkouts need no ignored files.
  projectFixtureRoot=await mkdtemp(resolve(root,'tests/workspace-fixtures-'));
  const projectDirectory=resolve(projectFixtureRoot,'project-context');
  await mkdir(projectDirectory);
  await writeFile(resolve(projectDirectory,'example.py'),'PROJECT_CONTEXT_MARKER = "workspace UI fixture"\n\ndef greet(name: str) -> str:\n    return f"Hello, {name}!"\n','utf8');
  // Folder upload traverses the filesystem; cold reads may exceed 30 seconds.
  await page.getByLabel('Choose project folder').setInputFiles(projectDirectory,{timeout:90000});
  await page.getByLabel(/project-context\/example.py/).check();
  await expect.poll(()=>state.workbench?.project?.[0]?.selected).toBe(true);
  await page.getByRole('complementary',{name:'Workspace'}).getByRole('button',{name:'Opex',exact:true}).click();
  await page.getByLabel('Opex PC project folder').fill(root);
  await page.getByRole('button',{name:'Connect / refresh project',exact:true}).click();
  await page.getByLabel('Opex mode',{exact:true}).selectOption('agent');
  await page.getByLabel('Opex PC request').fill('Test de terminal.');
  await page.getByRole('button',{name:'Propose next step',exact:true}).click();
  await expect(page.getByRole('button',{name:'Approve and run command',exact:true})).toBeVisible();
  expect(pcActions.filter(a=>a.action==='run')).toHaveLength(0);
  await page.getByRole('button',{name:'Approve and run command',exact:true}).click();
  await expect(page.getByLabel('Opex action log')).toContainText('OPEX_UI_OK');
  expect(pcActions.find(a=>a.action==='run').approved).toBe(true);
  await page.getByLabel('Opex PC request').fill('OPEX_INVALID_TEST');
  await page.getByRole('button',{name:'Propose next step',exact:true}).click();
  await expect(page.getByRole('alert')).toContainText('onverwachte goedkeuring');
  await expect(page.getByRole('button',{name:'Approve and run command',exact:true})).toHaveCount(0);
  expect(pcActions.filter(a=>a.action==='run')).toHaveLength(1);
  await page.getByLabel('Opex request').fill('Voeg een duidelijke uitvoer toe.');
  await page.getByRole('button',{name:'Propose change',exact:true}).click();
  await expect(page.getByLabel('Diff')).toContainText('OPEX_CHANGED');
  expect(state.workbench.project[0].text).toContain('PROJECT_CONTEXT_MARKER');
  await page.screenshot({path:resolve(root,'tests/orvyn-opex.png')});
  await page.getByRole('button',{name:'Apply to working copy'}).click();
  await expect.poll(()=>state.workbench.project[0].text).toContain('OPEX_CHANGED');
  await page.getByRole('button',{name:'Export working copy'}).click();
  await expect(page.getByText('Copy saved to USB:',{exact:false})).toBeVisible();
  await page.getByRole('button',{name:'Undo last change'}).click();
  await expect.poll(()=>state.workbench.project[0].text).toContain('PROJECT_CONTEXT_MARKER');
  await page.getByRole('button',{name:'Diff',exact:true}).click();
  await page.getByLabel('Original code').fill('console.log(1);');
  await page.getByLabel('New code').fill('console.log(2 + 3);');
  await expect(page.getByLabel('Diff')).toContainText('+ console.log(2 + 3);');
  await page.getByRole('button',{name:'Run JavaScript'}).click();
  await expect(page.locator('pre[role=status]')).toContainText('5',{timeout:35000});
  await expect(page.locator('code span[style]').first()).toBeVisible();
  await page.getByLabel('New code').fill('console.log([typeof fetch, typeof window, typeof require, typeof process].join("|"));');
  await page.getByRole('button',{name:'Run JavaScript'}).click();
  await expect(page.locator('pre[role=status]')).toContainText('undefined|undefined|undefined|undefined',{timeout:35000});
  await page.getByLabel('New code').fill('while (true) {}');
  await page.getByRole('button',{name:'Run JavaScript'}).click();
  await expect(page.locator('pre[role=status]')).toContainText(/interrupted|tijdslimiet/i,{timeout:35000});
  await page.getByRole('button',{name:'Tasks',exact:true}).click();
  await page.getByLabel('New task').fill('Een concrete afspraak');
  await page.getByRole('button',{name:'Add',exact:true}).click();
  await page.getByLabel('Task completed: Een concrete afspraak').check();
  await expect.poll(()=>state.workbench?.tasks[0]?.done).toBe(true);
  await page.getByRole('button',{name:'Extract tasks from conversation'}).click();
  await expect(page.getByText('Afspraak vanuit het gesprek',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Writing',exact:true}).click();
  await page.getByLabel('Writing text').fill('Ruwe tekst');
  await page.getByRole('button',{name:'Generate AI proposal'}).click();
  await expect(page.getByLabel('Writing proposal')).toHaveValue('Een verbeterde tekst.');
  await expect(page.getByLabel('Writing text')).toHaveValue('Ruwe tekst');
  await page.getByRole('button',{name:'Accept proposal'}).click();
  await expect(page.getByLabel('Writing text')).toHaveValue('Een verbeterde tekst.');
  await page.getByRole('button',{name:'Translate',exact:true}).click();
  await page.getByLabel('Translation source').fill('Hallo daar');
  await page.getByRole('button',{name:'Translate',exact:true}).last().click();
  await expect(page.getByLabel('Translation',{exact:true})).toHaveValue('Hello there.');
  await page.getByRole('button',{name:'Meeting',exact:true}).click();
  await page.getByLabel('Meeting transcript').fill('Jan maakt het verslag dinsdag.');
  await page.getByRole('button',{name:'Process notes'}).click();
  await expect(page.getByLabel('Meeting notes')).toHaveValue(/Actiepunten/);
  await page.getByRole('button',{name:'Create tasks from action items'}).click();
  await expect(page.getByText('Jan maakt het verslag dinsdag',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Learn',exact:true}).click();
  await page.getByLabel('Study material').fill('De oorlog begon in 1939.');
  await page.getByRole('button',{name:'Create flashcards / practice quiz'}).click();
  await page.getByLabel('Practice answer 1').fill('1939');
  await page.getByRole('button',{name:'Assess answer'}).click();
  await expect(page.getByText('Goed: 1939 is het juiste jaartal.',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Library',exact:true}).click();
  await page.getByLabel('Template name').fill('Mijn profiel');
  await page.getByLabel('Template text').fill('Usage korte zinnen.');
  await page.getByRole('button',{name:'Save profile'}).click();
  await page.getByLabel('Assistant profile').selectOption({label:'Mijn profiel'});
  await page.getByRole('button',{name:'Save prompt'}).click();
  await page.getByRole('button',{name:'Mijn profiel use'}).click();
  await expect(page.getByRole('textbox',{name:'Chat message'})).toHaveValue('Usage korte zinnen.');
  await page.getByRole('button',{name:'Close workspace'}).click();
  await page.getByRole('textbox',{name:'Chat message'}).fill('LIVE_TEST');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect(page.getByText('Live eerste deel',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Stop response'})).toBeVisible();
  await expect(page.getByText('Live eerste deel en tweede deel',{exact:true})).toBeVisible();
  expect(JSON.stringify(calls.at(-1))).toContain('PROJECT_CONTEXT_MARKER');
  expect(JSON.stringify(calls.at(-1))).toContain('Usage korte zinnen.');
  await expect(page.getByRole('button',{name:'Send message'})).toBeVisible();
  await page.getByLabel('Export conversation').selectOption('pdf');
  await expect.poll(()=>exports.length,{timeout:30000}).toBe(1);
  expect(Buffer.from(exports[0].base64,'base64').subarray(0,4).toString()).toBe('%PDF');
  const oldCount=state.chats.length;
  await page.getByRole('button',{name:'Branch',exact:true}).last().click();
  await expect.poll(()=>state.chats.length).toBe(oldCount+1);
  await page.getByRole('button',{name:'Pin',exact:true}).last().click();
  await expect(page.getByText('Pinned messages',{exact:true})).toBeVisible();
  const beforeEdit=state.chats.length;
  await page.getByRole('button',{name:'Edit',exact:true}).first().click();
  await page.getByRole('textbox',{name:'Edit message'}).fill('Mijn gewijzigde vraag');
  await page.getByRole('button',{name:'Edit and respond again'}).click();
  await expect.poll(()=>state.chats.length).toBe(beforeEdit+1);
  expect(state.chats.some(c=>c.messages.some(m=>m.content==='LIVE_TEST'))).toBe(true);
  await page.getByRole('button',{name:'Temporary chat',exact:true}).click();
  await page.getByRole('textbox',{name:'Chat message'}).fill('Mijn tijdelijke geheime tekst');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect(page.getByRole('button',{name:'Send message'})).toBeVisible();
  await page.waitForTimeout(500);
  expect(JSON.stringify(state)).not.toContain('Mijn tijdelijke geheime tekst');
  await page.getByRole('button',{name:'AI library',exact:false}).click();
  await page.getByRole('button',{name:'API providers',exact:true}).click();
  await expect(page.getByLabel('API base URL')).toHaveValue('https://api.openai.com/v1');
  await page.getByLabel('API provider',{exact:true}).selectOption('compatible');
  await expect(page.getByLabel('API base URL')).toBeEditable();
  await page.getByLabel('API provider',{exact:true}).selectOption('openrouter');
  await expect(page.getByLabel('API base URL')).toHaveValue('https://openrouter.ai/api/v1');
  await page.getByLabel('API model name').fill('Mijn OpenRouter');
  await page.getByLabel('API key',{exact:true}).fill('ui-test-secret');
  await page.getByRole('button',{name:'Test connection / load models'}).click();
  await expect(page.getByRole('status')).toContainText('2 model IDs');
  await page.getByLabel('API model ID').fill('openai/test-model');
  await page.getByRole('button',{name:'Save API model'}).click();
  await expect(page.getByRole('status')).toContainText('Saved');
  await expect(page.getByLabel('API key',{exact:true})).toHaveValue('');
  await page.screenshot({path:resolve(root,'tests/api-providers-settings.png')});
  await page.getByRole('button',{name:'Back to library'}).click();
  await page.getByRole('tab',{name:'Online / API',exact:true}).click();
  await expect(page.getByRole('dialog').getByText('Mijn OpenRouter',{exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Use',exact:true}).click();
  await expect(page.getByLabel('Smart model selection',{exact:true})).not.toBeChecked();
  await expect(page.getByText('Online / API: messages', {exact:false})).toBeVisible();
  await page.getByRole('textbox',{name:'Chat message'}).fill('Hallo via API');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect(page.getByText('Antwoord van api-test: 1939–1945.')).toBeVisible();
  expect(calls.at(-1).model).toBe('api-test');
  expect(JSON.stringify(state)).not.toContain('ui-test-secret');
  await page.getByRole('button',{name:'AI library',exact:false}).click();
  await page.getByRole('button',{name:'API providers',exact:true}).click();
  await page.getByRole('dialog').getByRole('button',{name:'Edit',exact:true}).click();
  await page.getByLabel('API model name').fill('Mijn API hernoemd');
  await page.getByRole('button',{name:'Save API model'}).click();
  await expect(page.getByRole('status')).toContainText('Saved');
  await page.getByRole('button',{name:'Mijn API hernoemd Remove API connection'}).click();
  await expect(page.getByRole('status')).toContainText('Conversations were preserved');
  expect(providers).toHaveLength(0);
  await page.getByRole('button',{name:'Close library'}).click();
  await expect(page.getByText('Antwoord van api-test: 1939–1945.')).toBeVisible();
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  const settings=page.getByRole('dialog',{name:'Orvyn · Settings'});
  await settings.getByRole('button',{name:'Startup',exact:true}).click();
  await expect(settings.getByText('Where does startup time go?')).toBeVisible({timeout:60000});
  await expect(settings.getByText('3.1 s',{exact:true})).toBeVisible();
  await settings.screenshot({path:resolve(root,'tests/orvyn-startup-diagnostics.png')});
  await settings.getByRole('button',{name:'Training',exact:true}).click();
  await expect(settings.getByText('112',{exact:true})).toBeVisible({timeout:60000});
  await expect(settings.getByText('48',{exact:true})).toBeVisible();
  await expect(settings.getByText('Do not apply:',{exact:false})).toBeVisible();
  await settings.screenshot({path:resolve(root,'tests/orvyn-training-dashboard.png')});
  await settings.getByRole('button',{name:'Compare',exact:true}).click();
  await expect(settings.getByText('Offline betekent zonder internetverbinding.',{exact:true})).toBeVisible();
  await expect(settings.getByText('Je hebt geen internetverbinding.',{exact:true})).toBeVisible();
  await settings.screenshot({path:resolve(root,'tests/orvyn-training-comparison.png')});
  await settings.getByRole('button',{name:'Examples',exact:true}).click();
  await settings.getByLabel('Training topic').selectOption('pc');
  await settings.getByLabel('Example set').selectOption('eval');
  await expect(settings.getByText('{"action":"read","path":"README.md"}',{exact:true})).toBeVisible();
  await settings.getByRole('button',{name:'Logo',exact:true}).click();
  for(const name of ['Orbit','Fold','Spark','Ribbon','Dialog']){
    await settings.getByRole('radio',{name:name+' as preference'}).check();
    await expect(settings.getByRole('status')).toContainText('Not approved or applied yet');
    await expect(settings.getByAltText('Current Orvyn logo')).toHaveAttribute('src','/orvyn-logo.png');
  }
  await expect.poll(()=>state.preferences.logoProposal).toBe('dialog');
  await settings.getByRole('button',{name:'Clear preference'}).click();
  await expect(settings.getByRole('status')).toContainText('No design selected yet');
  await settings.getByRole('radio',{name:'Orbit as preference'}).check();
  await page.setViewportSize({width:1400,height:1450});
  await settings.screenshot({path:resolve(root,'tests/orvyn-logo-options.png')});
  await page.setViewportSize({width:1400,height:1000});
  await settings.getByRole('button',{name:'General',exact:true}).click();
  await settings.getByLabel('Default response language').selectOption('English');
  await expect.poll(()=>state.preferences.language).toBe('English');
  await settings.getByRole('button',{name:'Appearance',exact:true}).click();
  await settings.getByLabel('Text size').selectOption('18');
  await settings.getByLabel('High contrast').check();
  await settings.getByLabel('Reduce motion').check();
  await expect(page.locator('html')).toHaveAttribute('data-contrast','true');
  await expect(page.locator('html')).toHaveCSS('font-size','18px');
  await settings.getByRole('button',{name:'Opex',exact:true}).click();
  await settings.getByLabel('Opex default instructions').fill('Behoud de bestaande code-stijl.');
  await expect.poll(()=>state.preferences.codeInstructions).toBe('Behoud de bestaande code-stijl.');
  await settings.getByRole('button',{name:'Storage',exact:true}).click();
  await settings.getByRole('button',{name:'Create backup',exact:true}).click();
  await expect(settings.getByRole('status')).toContainText('backups/orvyn-test.zip');
  await page.screenshot({path:resolve(root,'tests/orvyn-settings.png')});
  await settings.getByRole('button',{name:'General',exact:true}).click();
  await settings.getByLabel('Enter sends a message').uncheck();
  await settings.getByRole('button',{name:'Close settings'}).click();
  const previousCalls=calls.length;
  await page.getByRole('textbox',{name:'Chat message'}).fill('Niet verzenden');
  await page.getByRole('textbox',{name:'Chat message'}).press('End');
  await page.getByRole('textbox',{name:'Chat message'}).press('Enter');
  await expect(page.getByRole('textbox',{name:'Chat message'})).toHaveValue('Niet verzenden\n');
  expect(calls.length).toBe(previousCalls);
  await page.getByRole('textbox',{name:'Chat message'}).press('Control+Enter');
  await expect.poll(()=>calls.length).toBeGreaterThan(previousCalls);
  await expect(page.getByRole('button',{name:'Send message'})).toBeVisible();
  await page.reload();
  await page.locator('summary').filter({hasText:/^Chat options$/}).click();
  await expect(page.locator('html')).toHaveCSS('font-size','18px');
  await expect(page.locator('html')).toHaveAttribute('data-contrast','true');
  await expect(page).toHaveTitle('Orvyn · AI Model Hub');
  expect(state.preferences.logoProposal).toBe('orbit');
  expect(state.preferences.enterToSend).toBe(false);
  await page.getByRole('button',{name:'Tools & preferences',exact:true}).click();
  await page.getByLabel('Search online (Wikipedia)').check();
  await page.getByRole('textbox',{name:'Chat message'}).fill('Vat https://example.com samen');
  await page.getByRole('button',{name:'Send message'}).click();
  await expect.poll(()=>calls.some(c=>JSON.stringify(c.messages).includes('WEB_PAGE_MARKER'))).toBe(true);
  await expect(page.getByRole('button',{name:'Send message'})).toBeVisible();
  await page.getByLabel('Search online (Wikipedia)').uncheck();
  await page.getByRole('button',{name:'Tools & preferences',exact:true}).click();

  expect(errors).toEqual([]);
  console.log('UI integration passed, including real SSE chunks, syntax highlighting, QuickJS isolation/time limits, diffs, tasks, branching, pins, and temporary storage.');
}finally{
  try{await browser.close();}finally{
    try{await new Promise(r=>{server.close(r);server.closeAllConnections();});}finally{
      if(projectFixtureRoot)await rm(projectFixtureRoot,{recursive:true,force:true});
    }
  }
}

function makePdf(text){
  const stream=`BT /F1 12 Tf 40 700 Td (${text}) Tj ET`;
  const objects=['<< /Type /Catalog /Pages 2 0 R >>','<< /Type /Pages /Kids [3 0 R] /Count 1 >>','<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>','<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`];
  let pdf='%PDF-1.4\n',offsets=[0];objects.forEach((o,i)=>{offsets.push(Buffer.byteLength(pdf));pdf+=`${i+1} 0 obj\n${o}\nendobj\n`;});const xref=Buffer.byteLength(pdf);
  pdf+=`xref\n0 6\n0000000000 65535 f \n${offsets.slice(1).map(n=>String(n).padStart(10,'0')+' 00000 n \n').join('')}trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`;
  return Buffer.from(pdf);
}
