#!/usr/bin/env node
// Regression test for logo-lab.template.html: runs the page in jsdom and drives its controls.
// Usage: node test-lab.js <lab.html>   (needs the npm package jsdom; jsdom has no SVG layout, so getBBox and viewBox are stubbed)
const fs=require('fs'); const {JSDOM,VirtualConsole}=require('jsdom');
if(!process.argv[2]){console.error('usage: node test-lab.js <lab.html>');process.exit(2);}
const html=fs.readFileSync(process.argv[2],'utf8');
const errors=[]; const vc=new VirtualConsole(); vc.on('jsdomError',e=>errors.push(String(e.message||e))); vc.on('error',e=>errors.push(String(e)));
const dom=new JSDOM(html,{runScripts:'dangerously',pretendToBeVisual:true,virtualConsole:vc,
  beforeParse(w){ // jsdom has no SVG layout: give it the two things the page reads
    Object.defineProperty(w.SVGElement.prototype,'viewBox',{get(){const v=(this.getAttribute('viewBox')||'0 0 0 0').trim().split(/\s+/).map(Number);return {baseVal:{x:v[0],y:v[1],width:v[2],height:v[3]}};}});
    w.SVGElement.prototype.getBBox=function(){return {x:10,y:20,width:100,height:50};};
  }});
const w=dom.window, d=w.document; const $=id=>d.getElementById(id);
let pass=0, fail=0; const ok=(c,m)=>{ if(c){pass++;} else {fail++; console.log('FAIL:',m);} };
const near=(a,b,e=0.01)=>Math.abs(a-b)<e;
const fire=(el,type)=>el.dispatchEvent(new w.Event(type,{bubbles:true}));
const circles=()=>[...d.querySelectorAll('#mainlogos .logo-circle')];
const aro=s=>s.querySelector('circle[id^="aro"]');

ok(errors.length===0,'no script errors at load: '+errors.join(' | '));
ok(d.querySelectorAll('#chips .chip').length===9,'9 letter chips');
ok(d.querySelectorAll('#colorRows .color-row').length===3,'3 color rows');
ok(d.querySelectorAll('#presetBtns button').length===4,'4 preset buttons');
ok(d.querySelectorAll('#sizesBox .sizes-ver').length===3,'3 size strips');
ok(d.querySelectorAll('#sizesBox .sizes-ver:first-of-type figure').length===5,'5 figures per strip');
ok($('ringw').value==='100' && $('ringOut').textContent==='oficial','ring slider starts at 100 / oficial');
ok(circles().length===3,'3 circular logos inline');

// ring geometry before
const before=circles().map(s=>{const c=aro(s);const r=+c.getAttribute('r'),sw=+c.getAttribute('stroke-width');return {inner:r-sw/2,sw,stem:+s.getAttribute('data-stem')};});
before.forEach((b,i)=>ok(near(b.sw,b.stem,0.05),'v'+(i+1)+' official ring = 100% of stem ('+b.sw+' vs '+b.stem+')'));

// 1) paint a color role
$('c-teal').value='#ff0000'; fire($('c-teal'),'input');
ok($('mainlogos').closest('#stage').style.getPropertyValue('--papeleria-teal')==='#ff0000','css var --papeleria-teal set on stage');
ok($('cssOut').textContent.includes('--papeleria-teal: #ff0000'),'css snippet shows new teal');
ok($('h-teal').textContent==='#FF0000','hex readout updated');
ok($('chips').children[0].querySelector('.dot').style.background.replace(/\s/g,'')==='rgb(255,0,0)'||$('chips').children[0].querySelector('.dot').style.background==='#ff0000','teal chip dot updated');

// 2) ring width 150% keeps inner radius, viewBox grows
$('ringw').value='150'; fire($('ringw'),'input');
circles().forEach((s,i)=>{const c=aro(s);const r=+c.getAttribute('r'),sw=+c.getAttribute('stroke-width');const vb=s.getAttribute('viewBox').split(' ').map(Number);
  ok(near(sw,1.5*before[i].stem,0.02),'v'+(i+1)+' stroke = 150% stem');
  ok(near(r-sw/2,before[i].inner,0.01),'v'+(i+1)+' inner radius unchanged ('+(r-sw/2).toFixed(3)+' vs '+before[i].inner.toFixed(3)+')');
  ok(near(vb[0]+vb[2]/2,+c.getAttribute('cx'),0.01),'v'+(i+1)+' viewBox stays centered on the ring');
  ok(near(vb[2],vb[3],0.001) && vb[2]/2>=r+sw/2-0.001,'v'+(i+1)+' viewBox is square and holds the whole ring');});
ok($('ringOut').textContent==='150 %','ring readout 150 %');
const stripCircle=d.querySelector('#sizesBox .sizes-ver figure svg');
ok(stripCircle && near(+stripCircle.querySelector('circle[fill="none"][stroke]')?.getAttribute('stroke-width')||0,1.5*before[0].stem,0.02),'size strip rebuilt with the new ring width');

// 3) ring off -> ringless block with letters bbox + pad
$('ring-on').checked=false; fire($('ring-on'),'change');
circles().forEach((s,i)=>{const vb=s.getAttribute('viewBox').split(' ').map(Number);const pad=+s.getAttribute('data-pad');
  ok(aro(s).style.display==='none','v'+(i+1)+' ring hidden');
  ok(near(vb[0],10-pad,0.01)&&near(vb[1],20-pad,0.01)&&near(vb[2],100+2*pad,0.01)&&near(vb[3],50+2*pad,0.01),'v'+(i+1)+' viewBox = letters bbox + uniform pad');});
ok($('ringw').disabled && $('c-ring').disabled && $('disc-on').disabled,'ring controls disabled when ring is off');
// 4) ring back on, slider back to official
$('ring-on').checked=true; fire($('ring-on'),'change'); $('ringReset').click();
circles().forEach((s,i)=>{const c=aro(s);const sw=+c.getAttribute('stroke-width');ok(aro(s).style.display==='' && near(sw,before[i].sw,0.01),'v'+(i+1)+' ring restored to official');});
ok($('ringOut').textContent==='oficial','readout back to oficial');

// 5) single letter
d.querySelector('#chips .chip[data-n="3"]').click();
ok(d.querySelectorAll('#mainlogos path.is-sel').length===6,'letter 3 highlighted in all 6 logos (line + circular x3)');
ok(!$('letterRow').hidden,'letter color row visible');
$('c-letter').value='#000000'; fire($('c-letter'),'input');
ok($('stage').style.getPropertyValue('--papeleria-3')==='#000000','css var --papeleria-3 set');
ok($('cssOut').textContent.includes('--papeleria-3: #000000;  /* letra 3 (P) */'),'snippet lists letter 3');
$('letterClear').click();
ok($('stage').style.getPropertyValue('--papeleria-3')==='','per-letter override removed');

// 6) presets
d.querySelector('[data-preset="claro"]').click();
ok($('c-teal').value==='#2a8783' && $('c-yellow').value==='#b98600','preset "claro" applied');
ok($('stage').style.getPropertyValue('--papeleria-ring')==='#7a4327','preset ring color applied');
d.querySelector('[data-preset="original"]').click();
ok($('c-teal').value==='#7dc5c3' && $('stage').style.getPropertyValue('--papeleria-teal')==='','original preset clears overrides');

// 7) background + contrast readout
d.querySelector('#bg-blanco').checked=true; fire(d.querySelector('#bg-blanco'),'change');
ok($('stage').getAttribute('data-bg')==='blanco','background switched');
ok(/^[\d.]+:1$/.test($('r-teal').textContent) && $('r-teal').className.includes('low'),'teal flagged low contrast on white ('+$('r-teal').textContent+')');
d.querySelector('#bg-transparent').checked=true; fire(d.querySelector('#bg-transparent'),'change');
ok($('r-teal').textContent==='','no contrast readout on transparent stage');

// 8) size slider + reset all
$('size').value='300'; fire($('size'),'input'); ok($('stage').style.getPropertyValue('--w')==='300px','size var set');
$('disc-on').checked=true; fire($('disc-on'),'change'); ok($('stage').style.getPropertyValue('--papeleria-disc')!=='','disc var set');
$('resetAll').click();
ok($('stage').style.getPropertyValue('--w')==='420px' && $('c-teal').value==='#7dc5c3' && !$('disc-on').checked && $('ringOut').textContent==='oficial','reset all restores defaults');

ok(errors.length===0,'no script errors during interaction: '+errors.join(' | '));
console.log(fail?('FAILED '+fail+' / '+(pass+fail)):('all '+pass+' checks passed'));
process.exit(fail?1:0);
