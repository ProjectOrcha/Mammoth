import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Memory from './+page.svelte';
let component:ReturnType<typeof mount>;
const request=vi.fn();
const settle=async()=>{await new Promise(resolve=>setTimeout(resolve,0));flushSync();};
function fill(label:string,value:string) {
  const input=[...document.querySelectorAll('label')].find(l=>l.textContent?.startsWith(label))!.querySelector('input,textarea') as HTMLInputElement;
  input.value=value;input.dispatchEvent(new Event('input',{bubbles:true}));flushSync();
}
function submit(index:number) { document.querySelectorAll('form')[index].dispatchEvent(new Event('submit',{bubbles:true,cancelable:true})); }
const entry={project:'app',key:'handoff',title:'Next task',content:'Run tests',kind:'handoff',tags:['tests'],source:'src/api.rs',revision:3,updated_at:1};
beforeEach(async()=>{localStorage.clear();request.mockReset();vi.stubGlobal('fetch',request);component=mount(Memory,{target:document.body});await settle();});
afterEach(async()=>{await unmount(component);vi.unstubAllGlobals();document.body.replaceChildren();});
it('loads project-scoped memory and clears it when the project changes',async()=>{
  request.mockResolvedValue({ok:true,json:async()=>({memories:[entry],truncated:false})});
  fill('Project','app');submit(0);await settle();
  expect(request.mock.calls[0][0]).toContain('project=app');expect(document.body.textContent).toContain('Run tests');
  fill('Project','other');expect(document.querySelector('article')).toBeNull();
});
it('reads full entries and retains the revision and tags when saving edits',async()=>{
  request.mockResolvedValueOnce({ok:true,json:async()=>({memories:[entry],truncated:false})})
    .mockResolvedValueOnce({ok:true,json:async()=>entry})
    .mockResolvedValueOnce({ok:true,json:async()=>({...entry,revision:4})})
    .mockResolvedValueOnce({ok:true,json:async()=>({memories:[{...entry,revision:4}],truncated:false})});
  fill('Project','app');submit(0);await settle();
  [...document.querySelectorAll('button')].find(b=>b.textContent==='Read full entry / edit')!.click();await settle();
  fill('Context','Run the integration tests');submit(1);await settle();
  const body=JSON.parse(request.mock.calls[2][1].body);
  expect(body.expected_revision).toBe(3);expect(body.tags).toEqual(['tests']);expect(body.content).toBe('Run the integration tests');
  expect(document.body.textContent).toContain('Memory committed to disk.');
});
it('reports service failures instead of substituting simulated memories',async()=>{
  request.mockResolvedValue({ok:false,json:async()=>({message:'Memory service unavailable'})});
  fill('Project','app');submit(0);await settle();
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('Memory service unavailable');
  expect(document.querySelector('article')).toBeNull();
});
