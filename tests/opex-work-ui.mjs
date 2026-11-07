import {chromium,expect} from '../apps/web-ui/node_modules/@playwright/test/index.mjs';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {resolve} from 'node:path';
const root=fileURLToPath(new URL('../',import.meta.url));
const check=expect.configure({timeout:30000});
const server=createServer(async(req,res)=>{const name=req.url==='/'?'index.html':req.url.slice(1);if(!/^(index\.html|app\.(js|css)|orvyn-logo\.png|chunks\/[\w.-]+\.js)$/.test(name)){res.writeHead(404).end();return;}try{res.setHeader('Content-Type',name.endsWith('.css')?'text/css':name.endsWith('.html')?'text/html':name.endsWith('.png')?'image/png':'text/javascript');res.end(await readFile(resolve(root,'apps/web-ui/dist',name)));}catch{res.writeHead(500).end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch(process.env.ORVYN_BROWSER_CHANNEL==='bundled'?{headless:true}:{channel:process.env.ORVYN_BROWSER_CHANNEL||'msedge',headless:true});
try{
  const page=await browser.newPage({viewport:{width:1450,height:1000}}),errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));
  let saved={chats:[{id:'old-chat',title:'Bestaand gesprek',model:'coder',createdAt:Date.now(),messages:[{id:'m1',role:'assistant',content:'Je bestaande gesprek blijft bewaard.'}]}],activeChatId:'old-chat',activeModel:'coder',expandedFolders:['today'],revision:'0'},file='before',writes=0,starts=0,stopCalls=0,modelStep=0,jobStopped=false,slowJob=false;
  await page.route('**/api/**',async route=>{
    const req=route.request(),path=new URL(req.url()).pathname,body=req.postDataJSON();requests.push({path,body});let data={ok:true};
    if(path==='/api/storage'){if(req.method()==='PUT'){saved={...body,revision:String(Number(saved.revision)+1)};data={revision:saved.revision};}else data=saved;}
    else if(path==='/api/catalog')data={models:{coder:{name:'Test Coder',installed:true,category:'code',type:'chat',bytes:1e9,description:'Test'}},runtime:{active:null,phase:'idle',idle_seconds:1800},download:{status:'idle'},queue:[]};
    else if(path==='/api/index/scan')data={folders:[],files:[]};
    else if(path==='/api/hardware')data={ram_bytes:16e9,available_bytes:8e9,threads:8,cpu:'Test'};
    else if(path==='/api/opex/tree')data={folder:body.folder,files:[{path:'src',directory:true},{path:'src/main.py',directory:false}],truncated:false};
    else if(path==='/api/chat-stream'){
      const prompt=body.messages.at(-1).content;let content;
      if(prompt.includes('PLAN_ONLY'))content='1. Lees de code.\n2. Bespreek de wijziging.\n3. Voer alleen na akkoord uit.';
      else if(prompt.includes('INVALID_ACTION'))content=JSON.stringify({action:'run',command:'SHOULD_NOT_RUN',approved:true});
      else if(prompt.includes('READ_LIMIT'))content=JSON.stringify({action:'read',path:'src/main.py'});
      else if(prompt.includes('STOP_MODEL')){await new Promise(r=>setTimeout(r,1500));content=JSON.stringify({action:'write',path:'src/main.py',text:'should not save'});}
      else if(prompt.includes('STOP_COMMAND')){slowJob=true;content=JSON.stringify({action:'run',command:'Write-Output STARTED; Start-Sleep 30'});}
      else if(prompt.includes('REJECT_COMMAND'))content=JSON.stringify({action:'run',command:'Write-Output REJECT_ME'});
      else {content=JSON.stringify([{action:'read',path:'src/main.py'},{action:'write',path:'src/main.py',text:'print(42)'},{action:'run',command:'python src/main.py'},{answer:'De code is aangepast en het commando gaf 42 terug.'}][modelStep++]);}
      data={choices:[{message:{content}}]};
    }
    else if(path==='/api/opex/validate'){if(body.proposal.approved!==undefined){await route.fulfill({status:400,json:{error:'Onverwachte goedkeuring in modelvoorstel.'}});return;}data={proposal:body.proposal,valid:true};}
    else if(path==='/api/opex/action'){
      if(body.action==='read')data={text:file};else if(body.action==='write'){expect(body.approved).toBe(true);expect(body.base).toBe(file);writes++;file=body.text;data={backup:'USB/backups/opex-test'};}else throw Error('Unexpected action');
    }
    else if(path==='/api/opex/run'){starts++;expect(body.approved).toBe(true);data={id:'terminal-test'};}
    else if(path==='/api/opex/job')data=slowJob&&!jobStopped?{status:'running',output:'STARTED'}:{status:'finished',output:jobStopped?'STARTED':'42',exit_code:jobStopped?null:0,cancelled:jobStopped};
    else if(path==='/api/opex/stop'){stopCalls++;jobStopped=true;}
    await route.fulfill({json:data});
  });
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  await check(page.getByText('Je bestaande gesprek blijft bewaard.')).toBeVisible();
  const mode=page.getByRole('navigation',{name:'App mode'});
  await mode.getByRole('button',{name:'Work',exact:true}).click();
  await check(page.getByRole('heading',{name:'What shall we build?'})).toBeVisible();
  await page.screenshot({path:resolve(root,'tests/opex-work-home.png')});
  await page.getByRole('button',{name:'Start a task'}).click();
  await page.getByLabel('Work project folder').fill(root);
  await page.getByRole('button',{name:'Open project',exact:true}).click();
  await check(page.getByRole('button',{name:'src/main.py',exact:true})).toBeEnabled();
  await page.getByLabel('Work prompt',{exact:true}).fill('Verbeter de uitvoer en test het project.');
  await page.getByLabel('Send Work prompt').click();
  await check(page.getByRole('button',{name:'Approve and save'})).toBeVisible();
  expect(writes).toBe(0);expect(starts).toBe(0);
  await page.screenshot({path:resolve(root,'tests/opex-work-review.png')});
  await page.getByRole('button',{name:'Approve and save'}).click();
  await check(page.getByRole('button',{name:'Approve and run'})).toBeVisible();
  expect(writes).toBe(1);expect(file).toBe('print(42)');expect(starts).toBe(0);
  await page.getByRole('button',{name:'Approve and run'}).click();
  await check(page.getByText('De code is aangepast en het commando gaf 42 terug.')).toBeVisible();
  expect(starts).toBe(1);
  await page.getByRole('tab',{name:'Changes 1'}).click();
  await page.screenshot({path:resolve(root,'tests/opex-work-complete.png')});
  await check.poll(()=>saved.opexWork.tasks[0].status).toBe('done');
  await mode.getByRole('button',{name:'Chat',exact:true}).click();
  await check(page.getByText('Je bestaande gesprek blijft bewaard.')).toBeVisible();
  await page.reload();await mode.getByRole('button',{name:'Work',exact:true}).click();
  await check(page.getByText('De code is aangepast en het commando gaf 42 terug.')).toBeVisible();
  async function task(text){await page.getByRole('button',{name:'New task',exact:true}).click();await page.getByLabel('Work prompt',{exact:true}).fill(text);await page.getByLabel('Send Work prompt').click();}
  await task('REJECT_COMMAND');
  await check(page.getByRole('button',{name:'Approve and run'})).toBeVisible();
  await page.getByRole('button',{name:'Reject',exact:true}).click();expect(starts).toBe(1);
  await task('REJECT_COMMAND_SWITCH');
  await check(page.getByRole('button',{name:'Approve and run'})).toBeVisible();
  await page.getByRole('button',{name:'New task',exact:true}).click();
  await page.getByRole('complementary',{name:'Opex tasks'}).getByRole('button',{name:/REJECT_COMMAND_SWITCH/}).click();
  await check(page.getByText('Proposal was not executed.',{exact:false})).toBeVisible();
  await check(page.getByLabel('Review action')).toHaveCount(0);expect(starts).toBe(1);
  await task('INVALID_ACTION');
  await check(page.getByRole('alert')).toContainText('Onverwachte goedkeuring');expect(starts).toBe(1);
  await page.getByRole('button',{name:'New task',exact:true}).click();
  await page.getByLabel('Work mode',{exact:true}).selectOption('plan');await page.getByLabel('Work prompt',{exact:true}).fill('PLAN_ONLY');
  const actionCount=requests.filter(r=>['/api/opex/action','/api/opex/run'].includes(r.path)).length;
  await page.getByLabel('Send Work prompt').click();await check(page.getByText('1. Lees de code.',{exact:false})).toBeVisible();
  expect(requests.filter(r=>['/api/opex/action','/api/opex/run'].includes(r.path)).length).toBe(actionCount);
  await task('STOP_COMMAND');await check(page.getByRole('button',{name:'Approve and run'})).toBeVisible();
  await page.getByRole('button',{name:'Approve and run'}).click();await check(page.getByLabel('Live terminal output')).toContainText('STARTED');
  await page.getByLabel('Stop Opex').click();await check(page.getByLabel('Send Work prompt')).toBeVisible();expect(stopCalls).toBe(1);
  await task('STOP_MODEL');await check(page.getByLabel('Stop Opex')).toBeVisible();await page.getByLabel('Stop Opex').click();
  await check(page.getByLabel('Send Work prompt')).toBeVisible();await page.waitForTimeout(1800);expect(writes).toBe(1);await check(page.getByLabel('Review action')).toHaveCount(0);
  await task('READ_LIMIT');await check(page.getByText('Eight steps completed.',{exact:false})).toBeVisible();
  const active=saved.opexWork.tasks.find(t=>t.id===saved.opexWork.active);await check.poll(()=>saved.opexWork.tasks.find(t=>t.id===active.id).status).toBe('stopped');expect(active.events.filter(e=>e.kind==='read')).toHaveLength(8);
  await page.setViewportSize({width:900,height:850});await page.screenshot({path:resolve(root,'tests/opex-work-compact.png')});
  expect(errors).toEqual([]);expect(saved.chats[0].messages[0].content).toBe('Je bestaande gesprek blijft bewaard.');
  console.log('Opex Work UI passed: separate chat, project tree, read → review → write → terminal → answer, persistence, rejection, plan-only, model/terminal stop and step limit.');
}finally{await browser.close();server.close();}
