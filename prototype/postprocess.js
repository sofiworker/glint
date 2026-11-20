/* SPDX-License-Identifier: GPL-3.0-only. Interactive specification; not the native editing engine. */
(() => {
  'use strict';
  const root=document.getElementById('glint-postprocess');if(!root)return;
  const $=s=>root.querySelector(s),$$=s=>[...root.querySelectorAll(s)],clone=v=>JSON.parse(JSON.stringify(v));
  const clamp=(v,a,b)=>Math.max(a,Math.min(b,v));
  const esc=v=>String(v).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const names={select:'选择',crop:'裁剪',shape:'标注',rect:'矩形',arrow:'箭头',pen:'画笔',highlight:'高亮',text:'文字',redact:'遮盖',mosaic:'像素化',adjust:'调整',clip:'片段',audio:'声音'};
  const icon=k=>GlintIcons.svg(k);
  const symbols=()=>{hideTooltip();$$('button,summary,[data-crop-handle]').forEach(n=>n.classList.add('cursor-interaction'));GlintIcons.render(root);};
  // One input-transparent overlay for the entire workbench. Keeping hints out
  // of toolbar rows prevents the next opaque panel from covering their text.
  const tooltip=document.createElement('div');tooltip.className='gp-tooltip';tooltip.hidden=true;tooltip.role='tooltip';root.append(tooltip);
  let tooltipAnchor=null,tooltipTimer=null,tooltipPoint={x:0,y:0};
  function hideTooltip(){clearTimeout(tooltipTimer);tooltipTimer=null;tooltip.hidden=true;tooltipAnchor=null;}
  function placeTooltip(){
    const box=tooltip.getBoundingClientRect(),gap=8;
    const x=clamp(tooltipPoint.x,gap,Math.max(gap,innerWidth-box.width-gap));
    let y=tooltipPoint.y+gap;if(y+box.height>innerHeight-gap)y=tooltipPoint.y-box.height-gap;
    tooltip.style.left=x+'px';tooltip.style.top=clamp(y,gap,Math.max(gap,innerHeight-box.height-gap))+'px';
  }
  root.addEventListener('pointerover',e=>{
    const button=e.target.closest('button[data-tooltip],button.gp-icon[aria-label]');
    if(!button||button===tooltipAnchor)return;hideTooltip();tooltipAnchor=button;tooltipPoint={x:e.clientX,y:e.clientY};
    tooltipTimer=setTimeout(()=>{if(tooltipAnchor!==button||!button.isConnected)return;tooltip.textContent=button.dataset.tooltip||button.getAttribute('aria-label');tooltip.hidden=false;placeTooltip();},500);
  });
  root.addEventListener('pointerout',e=>{if(tooltipAnchor?.contains(e.target)&&!tooltipAnchor.contains(e.relatedTarget))hideTooltip();});
  root.addEventListener('pointermove',e=>{tooltipPoint={x:e.clientX,y:e.clientY};if(!tooltip.hidden)placeTooltip();});
  root.addEventListener('pointerdown',hideTooltip);root.addEventListener('keydown',hideTooltip);
  window.addEventListener('blur',hideTooltip);window.addEventListener('resize',hideTooltip);window.addEventListener('scroll',hideTooltip,true);
  const design={appearance:'system',accent:'#4b5cfa'};
  const initial=(kind,w=1280,h=800,duration=32)=>({width:w,height:h,crop:{x:0,y:0,w,h},rotation:0,flip:false,brightness:100,contrast:100,saturation:100,marks:kind==='image'?[{id:1,kind:'redact',x:325,y:710,w:286,h:37,color:'#24262b',line:4},{id:2,kind:'rect',x:190,y:420,w:720,h:65,color:'#4b5cfa',line:4}]:[{id:3,kind:'text',x:780,y:130,w:290,h:48,text:'这一步是重点',font:40,color:'#4b5cfa',line:4,from:4,to:12}],clips:[{id:1,start:0,end:duration}],volume:100,muted:false});
  const assets=[{id:'image',kind:'image',name:'截图 11-16.png',w:1280,h:800,duration:0,url:null,s:initial('image'),history:[],redo:[],time:0},{id:'video',kind:'video',name:'操作演示 11-16.mp4',w:1280,h:800,duration:32,url:null,s:initial('video'),history:[],redo:[],time:8}];
  assets.forEach(a=>a.baseline=clone(a.s));
  let asset=assets[0],stage='result',category='select',tool='select',selected=null,selectedClip=1;
  let resultVisible=false,dialog='',menu=false,compare=false,zoom=1,pan={x:0,y:0},space=false,textEntry=null;
  let color='#4b5cfa',line=4,text='重点在这里',font=40,cropRatio='free',cropDraft={x:.08,y:.1,w:.84,h:.8};
  let drag=null,rangeBefore=null,playing=false,timer=null,toastTimer=null,returnFocus=null,nextId=10,pendingAssetId=null,cropBefore=null,lastMarks='';
  const state=()=>asset.s,mode=()=>asset.kind,total=()=>state().clips.reduce((n,c)=>n+c.end-c.start,0);
  const dims=()=>compare?[asset.w,asset.h]:state().rotation%180?[state().height,state().width]:[state().width,state().height];
  const mark=()=>state().marks.find(m=>m.id===selected);
  const dirty=()=>JSON.stringify(state())!==JSON.stringify(asset.baseline);
  const stamp=v=>{v=Math.max(0,v);return `${Math.floor(v/60).toString().padStart(2,'0')}:${Math.floor(v%60).toString().padStart(2,'0')}.${Math.floor((v%1)*10)}`;};
  function checkpoint(before=state()){asset.history.push(clone(before));if(asset.history.length>40)asset.history.shift();asset.redo=[];}
  function change(fn){checkpoint();fn();asset.time=clamp(asset.time,0,total());render();persist();}
  function persist(){window.openai?.setWidgetState({modelContent:{design:'Glint workbench v7',stage,mode:mode(),tool:category,appearance:design.appearance},privateContent:{assetId:asset.id,activeTool:tool,resultVisible,zoom}})?.catch(()=>{});}
  function notify(message){clearTimeout(toastTimer);$('.gp-toast').textContent=message;$('.gp-toast').hidden=false;toastTimer=setTimeout(()=>{$('.gp-toast').hidden=true;},3600);}
  function stop(){playing=false;clearInterval(timer);timer=null;$('.gp-local-video').pause();}
  function cancelCropDraft(){if(cropBefore){asset.s=cropBefore;cropBefore=null;}}
  function switchAsset(id){const next=assets.find(a=>a.id===id);if(!next)return;cancelCropDraft();stop();clearTimeout(toastTimer);$('.gp-toast').hidden=true;asset=next;stage='editor';category=mode()==='video'?'clip':'select';tool='select';selected=null;selectedClip=state().clips[0].id;zoom=1;pan={x:0,y:0};compare=false;dialog='';menu=false;textEntry=null;render();$('.gp-toolrail button[aria-pressed=true]')?.focus({preventScroll:true});persist();}
  function choose(id){if(textEntry)commitText();cancelCropDraft();stop();menu=false;compare=false;category=id;tool=['select','crop','rect','arrow','pen','highlight','text','redact','mosaic'].includes(id)?id:'select';selected=null;if(id==='redact')color='#24262b';if(id==='crop'){cropBefore=clone(state());zoom=1;pan={x:0,y:0};cropDraft={x:.08,y:.1,w:.84,h:.8};cropRatio='free';}render();$(`.gp-toolrail [data-category=${id}]`)?.focus({preventScroll:true});persist();}
  function renderRail(){
    const tools=['select','crop','rect','arrow','pen','text','highlight','redact','mosaic','adjust'];
    $('.gp-toolrail').innerHTML=tools.map(id=>`<button class="gp-icon" type="button" data-category="${id}" aria-pressed="${category===id}" aria-label="${names[id]}" data-tooltip="${names[id]}">${icon(id)}</button>`).join('');
  }
  const number=(label,key,value,min=0,max=16384,step=1)=>`<label>${label}<input type="number" data-field="${key}" value="${Math.round(value*100)/100}" min="${min}" max="${max}" step="${step}" required></label>`;
  const pair=(label,value)=>`<div class="gp-property-pair"><span>${label}</span><b>${value}</b></div>`;
  const colors=['#4b5cfa','#ed6262','#e6b443','#24262b','#ffffff'];
  function swatches(value){return `<div class="gp-colors" role="group" aria-label="标注颜色">${colors.map(c=>`<button type="button" class="gp-swatch" style="--swatch:${c}" data-color="${c}" aria-label="颜色 ${c}" aria-pressed="${value===c}"><span></span></button>`).join('')}</div>`;}
  function renderProperties(){
    const s=state(),m=textEntry?.m||mark(),kind=m?.kind||tool;let html='';
    const action=(id,label,ic,extra='')=>`<button type="button" ${ic?'class="gp-icon"':''} data-action="${id}" aria-label="${label}" data-tooltip="${label}" ${extra}>${ic?icon(ic):label}</button>`;
    const field=(label,key,value,min=0,max=16384,step=.1)=>`<label class="gp-compact-field">${label}<input type="number" data-field="${key}" value="${Math.round(value*100)/100}" min="${min}" max="${max}" step="${step}" required></label>`;
    const divider='<span class="gp-divider"></span>';
    if(category==='crop'){
      html=`<label class="gp-inline-label">比例<select data-field="crop-ratio">${[['free','自由'],['1:1','1:1'],['4:3','4:3'],['16:9','16:9']].map(([v,l])=>`<option value="${v}" ${cropRatio===v?'selected':''}>${l}</option>`).join('')}</select></label>${divider}${action('rotate','旋转 90°','rotate')}${action('flip','水平翻转','flip')}<span class="gp-context-spacer"></span><button type="button" data-action="cancel-crop">取消</button><button type="button" class="gp-primary" data-action="apply-crop">应用</button>`;
    }else if(category==='adjust'){
      html=['brightness','contrast','saturation'].map((k,i)=>`<label class="gp-adjust-field"><span>${['亮度','对比度','饱和度'][i]}<b data-value="${k}">${s[k]}%</b></span><input type="range" data-field="${k}" min="0" max="200" value="${s[k]}" aria-label="${['亮度','对比度','饱和度'][i]}"></label>`).join('')+'<span class="gp-context-spacer"></span><button type="button" data-action="reset-adjust">还原调整</button>';
    }else if(category==='audio'){
      html=`<label class="gp-check"><input type="checkbox" data-field="muted" ${s.muted?'checked':''}>静音</label>${divider}<label class="gp-adjust-field"><span>音量<b data-value="volume">${s.volume}%</b></span><input type="range" data-field="volume" min="0" max="100" value="${s.volume}" aria-label="音量"></label>`;
    }else if(category==='clip'&&!m){
      const c=s.clips.find(c=>c.id===selectedClip)||s.clips[0];
      html=`<span class="gp-context-name">片段 ${s.clips.indexOf(c)+1}</span>${divider}${field('起点','clip-start',c.start,0,c.end-.1)}${field('终点','clip-end',c.end,c.start+.1,asset.duration)}<span class="gp-context-unit">秒</span>`;
    }else{
      if(kind!=='select')html=`<span class="gp-context-name">${names[kind]}</span>`;
      if(kind!=='select'){
        html+=divider;
        if(kind!=='mosaic')html+=swatches(m?.color||color);
        if(kind==='text')html+=`<label class="gp-inline-label">字号<select data-field="font">${[...new Set([24,32,40,48,64,96,Math.round((m?.font||font)*100)/100])].sort((a,b)=>a-b).map(v=>`<option value="${v}" ${Math.abs((m?.font||font)-v)<.01?'selected':''}>${v}</option>`).join('')}</select></label>${m&&!textEntry?action('edit-text','编辑文字'):''}`;
        else if(!['redact','mosaic'].includes(kind))html+=`<label class="gp-inline-label">线宽<select data-field="line">${[2,4,8,12].map(v=>`<option value="${v}" ${(m?.line||line)===v?'selected':''}>${v} px</option>`).join('')}</select></label>`;
        if(kind==='mosaic')html+='<span class="gp-context-note">像素化不适合保密内容</span>';
      }
      if(m&&!textEntry&&mode()==='video')html+=`${divider}${field('出现','mark-from',m.from||0,0,total())}${field('结束','mark-to',m.to??total(),.1,total())}<span class="gp-context-unit">秒</span>`;
      if(m&&!textEntry)html+=`<span class="gp-context-spacer"></span>${action('duplicate','复制对象','copy')}${action('delete','删除对象','trash')}`;

    }
    $('.gp-context').innerHTML=html;
    $('.gp-context').hidden=!html;
  }

  function node(tag,attrs){const n=document.createElementNS('http://www.w3.org/2000/svg',tag);Object.entries(attrs).forEach(([k,v])=>n.setAttribute(k,v));return n;}
  function bounds(m){return{x:Math.min(m.x,m.x+m.w),y:Math.min(m.y,m.y+m.h),w:Math.abs(m.w),h:Math.abs(m.h)};}
  function renderSelection(svg){
    svg.querySelectorAll('.gp-selection,[data-handle]').forEach(n=>n.remove());const m=mark();
    if(!m||textEntry?.id===m.id||mode()==='video'&&(asset.time<(m.from||0)||asset.time>(m.to??total())))return;
    const b=bounds(m),scale=asset.w/Math.max(1,svg.getBoundingClientRect().width),r=6*scale;
    svg.append(node('rect',{class:'gp-selection',x:b.x,y:b.y,width:b.w,height:b.h,fill:'none',stroke:design.accent,'stroke-width':scale}));
    for(const [h,x,y]of [['nw',b.x,b.y],['ne',b.x+b.w,b.y],['sw',b.x,b.y+b.h],['se',b.x+b.w,b.y+b.h]])svg.append(node('rect',{'data-handle':h,x:x-r/2,y:y-r/2,width:r,height:r,fill:'#fff',stroke:design.accent,'stroke-width':scale}));
  }
  function renderMarks(){
    const svg=$('.gp-annotations');svg.setAttribute('viewBox',`0 0 ${asset.w} ${asset.h}`);svg.style.visibility=compare?'hidden':'visible';
    const signature=asset.id+JSON.stringify(state().marks.filter(m=>textEntry?.id!==m.id&&(mode()!=='video'||asset.time>=(m.from||0)&&asset.time<=(m.to??total()))));
    if(signature===lastMarks){renderSelection(svg);return;}lastMarks=signature;svg.replaceChildren();
    for(const m of state().marks){
      if(textEntry?.id===m.id)continue;
      if(mode()==='video'&&(asset.time<(m.from||0)||asset.time>(m.to??total())))continue;
      const g=node('g',{'data-mark':m.id}),c=m.color,l=m.line||4;
      if(['rect','redact','mosaic','highlight'].includes(m.kind)){
        g.append(node('rect',{x:m.x,y:m.y,width:m.w,height:m.h,fill:m.kind==='rect'?'none':c,stroke:m.kind==='rect'?c:'none','stroke-width':l,opacity:m.kind==='highlight'?.3:1}));
        if(m.kind==='rect')g.append(node('rect',{x:m.x,y:m.y,width:m.w,height:m.h,fill:'none',stroke:'transparent','stroke-width':18}));
        if(m.kind==='mosaic')for(let x=m.x;x<m.x+m.w;x+=20)for(let y=m.y;y<m.y+m.h;y+=20)g.append(node('rect',{x,y,width:Math.min(20,m.x+m.w-x),height:Math.min(20,m.y+m.h-y),fill:['#717988','#c3c7d0','#929ba9','#d6d9e0'][(Math.floor(x/20)*7+Math.floor(y/20)*3)%4]}));
      }else if(m.kind==='arrow'){
        const x=m.x+m.w,y=m.y+m.h,a=Math.atan2(m.h,m.w),d=23+l;
        g.append(node('line',{x1:m.x,y1:m.y,x2:x,y2:y,stroke:c,'stroke-width':l,'stroke-linecap':'round'}));
        g.append(node('path',{d:`M ${x-d*Math.cos(a-.5)} ${y-d*Math.sin(a-.5)} L ${x} ${y} L ${x-d*Math.cos(a+.5)} ${y-d*Math.sin(a+.5)}`,fill:'none',stroke:c,'stroke-width':l,'stroke-linecap':'round','stroke-linejoin':'round'}));
        g.append(node('line',{x1:m.x,y1:m.y,x2:x,y2:y,stroke:'transparent','stroke-width':20}));
      }else if(m.kind==='pen')g.append(node('polyline',{points:m.points.map(p=>p.join(',')).join(' '),fill:'none',stroke:c,'stroke-width':l,'stroke-linecap':'round','stroke-linejoin':'round'}));
      else if(m.kind==='text'){const n=node('text',{x:m.x,y:m.y+(m.font||40),fill:c,'font-size':m.font||40,'font-weight':m.weight||500,'font-family':'Segoe UI,Microsoft YaHei UI,sans-serif'});n.textContent=m.text;g.append(n);}
      svg.append(g);
    }
    renderSelection(svg);
  }
  function renderImage(){
    if(stage!=='editor')return;
    const s=state(),[w,h]=dims(),frame=$('.gp-image-frame'),source=$('[data-source]'),stageBox=$('.gp-stage').getBoundingClientRect();
    const fit=Math.max(30,Math.min(stageBox.width-60,(stageBox.height-70)*w/h,760));frame.style.width=`${fit}px`;frame.style.maxWidth='none';frame.style.aspectRatio=`${w}/${h}`;frame.style.transform=`translate(${pan.x}px,${pan.y}px) scale(${zoom})`;
    const width=fit,height=fit*h/w,crop=compare?{x:0,y:0,w:asset.w,h:asset.h}:s.crop,rot=compare?0:s.rotation;
    const sx=(rot%180?height:width)/crop.w,sy=(rot%180?width:height)/crop.h;
    source.style.width=`${asset.w*sx}px`;source.style.height=`${asset.h*sy}px`;
    source.style.transform=`translate(-50%,-50%) rotate(${rot}deg) scaleX(${!compare&&s.flip?-1:1}) translate(${(asset.w/2-crop.x-crop.w/2)*sx}px,${(asset.h/2-crop.y-crop.h/2)*sy}px)`;
    // The filter is a preview; native export must composite actual pixels separately.
    $$('.gp-sample,.gp-local-image,.gp-local-video').forEach(n=>n.style.filter=compare?'none':`brightness(${s.brightness}%) contrast(${s.contrast}%) saturate(${s.saturation}%)`);
    $('.gp-crop').hidden=category!=='crop'||compare;$('.gp-original-label').hidden=!compare;
    renderCrop();renderMarks();renderTextEntry();
    $('[data-zoom]').textContent=zoom===1?'适应':`${Math.round(zoom*fit/w*100)}%`;
    $('[data-hint]').textContent='';
  }
  function beginText(m,isNew=false){
    stop();selected=m.id;category='text';tool='text';textEntry={id:m.id,m:clone(m),isNew};render();
    const input=$('.gp-text-entry textarea');input?.focus({preventScroll:true});input?.select();
  }
  function renderTextEntry(){
    let n=$('.gp-text-entry');if(!textEntry){n?.remove();return;}
    const m=textEntry.m;
    if(!n||Number(n.dataset.editId)!==m.id){n?.remove();n=document.createElement('div');n.className='gp-text-entry';n.dataset.editId=m.id;n.innerHTML='<textarea rows="1" aria-label="编辑标注文字"></textarea><div class="gp-text-entry-actions"><button type="button" data-action="cancel-text">取消</button><button type="button" data-action="commit-text">完成</button></div>';n.querySelector('textarea').value=m.text;$('[data-source]').append(n);}
    n.style.left=`${m.x/asset.w*100}%`;n.style.top=`${m.y/asset.h*100}%`;n.style.width=`${Math.min(asset.w-m.x,Math.max(260,m.w+80))/asset.w*100}%`;n.style.setProperty('--text-color',m.color);
    n.querySelector('textarea').style.fontSize=`${(m.font||font)/asset.w*100}cqw`;
  }
  function commitText(){
    if(!textEntry)return;const entry=textEntry,value=$('.gp-text-entry textarea')?.value.trim()||'';textEntry=null;
    if(!value){selected=entry.isNew?null:entry.id;tool='select';category='select';render();return;}
    text=value;const updated={...entry.m,text:value,w:Math.max(30,value.length*(entry.m.font||40)*.9)};
    if(!entry.isNew&&JSON.stringify(updated)===JSON.stringify(state().marks.find(m=>m.id===entry.id))){tool='select';category='select';render();return;}
    change(()=>{if(entry.isNew)state().marks.push(updated);else Object.assign(state().marks.find(m=>m.id===entry.id),updated);selected=entry.id;tool='select';category='select';});
  }
  function cancelText(){if(!textEntry)return;selected=textEntry.isNew?null:textEntry.id;textEntry=null;tool='select';category='select';render();$('.gp-toolrail [data-category=select]')?.focus({preventScroll:true});}
  function renderCrop(){
    const c=cropDraft,n=$('.gp-crop-box');n.style.left=`${c.x*100}%`;n.style.top=`${c.y*100}%`;n.style.width=`${c.w*100}%`;n.style.height=`${c.h*100}%`;const[w,h]=dims();$('.gp-crop-size').textContent=`${Math.round(w*c.w)} × ${Math.round(h*c.h)}`;
  }
  function sourceTime(){let offset=0;for(const c of state().clips){const d=c.end-c.start;if(asset.time<offset+d)return c.start+asset.time-offset;offset+=d;}return state().clips.at(-1).end;}
  function renderTime(){
    const duration=total();asset.time=clamp(asset.time,0,duration);$('[data-time]').textContent=stamp(asset.time);$('[data-duration]').textContent=` / ${stamp(duration)}`;
    $('[data-field=seek]').max=duration;$('[data-field=seek]').value=asset.time;$('.gp-playhead').style.left=`${asset.time/duration*100}%`;
    $('[data-action=play]').innerHTML=icon(playing?'pause':'play');$('[data-action=play]').setAttribute('aria-label',playing?'暂停':'播放');
    let offset=0,canSplit=false;for(const c of state().clips){if(asset.time>offset+.1&&asset.time<offset+c.end-c.start-.1)canSplit=true;offset+=c.end-c.start;}
    $$('[data-action=split]').forEach(n=>n.disabled=!canSplit);$$('[data-action=delete-clip]').forEach(n=>n.disabled=state().clips.length<2);
    const v=$('.gp-local-video');v.muted=state().muted;v.volume=state().volume/100;if(mode()==='video'&&asset.url&&v.readyState>=1&&Math.abs(v.currentTime-sourceTime())>.1)v.currentTime=sourceTime();
    renderMarks();symbols();
  }
  function renderTimeline(){
    const s=state(),duration=total();$('.gp-ruler').innerHTML=Array.from({length:5},(_,i)=>`<span style="left:${i*25}%">${stamp(duration*i/4).slice(0,-2)}</span>`).join('');
    $('.gp-clip-track').innerHTML=s.clips.map((c,i)=>`<button type="button" class="gp-clip" data-clip="${c.id}" aria-pressed="${c.id===selectedClip}" aria-label="片段 ${i+1}，${(c.end-c.start).toFixed(1)} 秒" style="flex-basis:${(c.end-c.start)/duration*100}%"><span class="gp-clip-frames" aria-hidden="true">${Array.from({length:8},()=>'<span><i></i><i></i><i></i></span>').join('')}</span><span class="gp-clip-name">片段 ${i+1}</span></button>`).join('');
    $('.gp-mark-track').innerHTML=s.marks.filter(m=>(m.from||0)<duration).map(m=>`<button type="button" class="gp-timed-mark" data-timed-object="${m.id}" aria-label="${names[m.kind]}，${stamp(m.from||0)} 至 ${stamp(Math.min(m.to??duration,duration))}" style="left:${(m.from||0)/duration*100}%;width:${(Math.min(m.to??duration,duration)-(m.from||0))/duration*100}%">${m.kind==='text'?esc(m.text):names[m.kind]}</button>`).join('');
    $('[data-timeline-summary]').textContent=`${s.clips.length} 个片段 · ${duration.toFixed(1)} 秒`;
    $('[data-audio-button]').setAttribute('aria-pressed',String(category==='audio'));$('[data-audio-button]').innerHTML=`${icon(s.muted?'muted':'audio')}<span>${s.muted?'已静音':'声音'}</span>`;renderTime();
  }
  function renderMedia(){
    const img=$('.gp-local-image'),video=$('.gp-local-video');$('.gp-sample').hidden=!!asset.url;img.hidden=!asset.url||mode()!=='image';video.hidden=!asset.url||mode()!=='video';
    if(asset.url&&mode()==='image'&&img.getAttribute('src')!==asset.url)img.src=asset.url;
    if(asset.url&&mode()==='video'&&video.getAttribute('src')!==asset.url)video.src=asset.url;
  }
  function render(){
    const active=document.activeElement,focusKey=active?.closest('#glint-postprocess')?['data-field','data-category','data-object','data-tool'].find(k=>active.hasAttribute(k)):null,focusValue=focusKey?active.getAttribute(focusKey):null;
    root.dataset.stage=stage;root.dataset.mode=mode();root.dataset.theme=design.appearance;root.style.setProperty('--gp-accent',design.accent);
    $('.gp-desktop-view').hidden=stage!=='result';$('.gp-window').hidden=stage!=='editor';$('.gp-result-float').hidden=!resultVisible;
    $$('[data-preview]').forEach(n=>n.setAttribute('aria-pressed',String(n.dataset.preview===(stage==='result'?'result':mode()))));
    $('[data-filename]').textContent=asset.name;$('[data-dirty]').textContent=dirty()?'已修改':'';
    $('[data-action=undo]').disabled=!asset.history.length;$('[data-action=redo]').disabled=!asset.redo.length;$('[data-action=copy]').hidden=mode()==='video';
    $('.gp-video').hidden=mode()!=='video';$('[data-export-label]').textContent=mode()==='video'?'导出视频':'保存副本';
    $('[data-editor-info]').textContent=`${mode()==='image'?'PNG':'MP4 · 30 FPS'} · ${dims().join(' × ')}`;
    $('[data-action=compare]').setAttribute('aria-pressed',String(compare));
    $('[data-view-note]').textContent='';
    renderRail();renderMedia();renderProperties();renderImage();renderTimeline();renderDialog();renderMenu();renderResult();symbols();
    if(focusKey&&!active.isConnected)$(`[${focusKey}="${focusValue}"]`)?.focus({preventScroll:true});
  }
  function renderResult(){
    $('[data-result-status]').textContent=mode()==='video'?'录屏已结束':'已复制到剪贴板';$('[data-result-kind]').textContent=mode()==='video'?'录屏':'截图';
    $('.gp-result-thumbnail').querySelector('.gp-result-annotations')?.remove();
    const marks=$('.gp-image-source .gp-annotations').cloneNode(true);marks.setAttribute('class','gp-result-annotations');marks.style.visibility='visible';marks.querySelectorAll('.gp-selection,[data-handle]').forEach(n=>n.remove());$('.gp-result-thumbnail').append(marks);
  }
  function dialogHead(title){return `<div class="gp-dialog-head"><strong>${title}</strong><button type="button" class="gp-icon" data-action="dismiss" aria-label="关闭对话框">${icon('close')}</button></div>`;}
  function renderMenu(){
    const n=$('.gp-more-menu');n.hidden=!menu;$('[data-action=more]').setAttribute('aria-expanded',String(menu));
    n.innerHTML=menu?`<button type="button" data-action="resize-dialog">${icon('scaling')}调整尺寸</button><button type="button" data-action="objects">${icon('layers')}标注对象</button><hr><button type="button" data-action="open-file">${icon('folder-open')}打开其他文件…</button>`:'';
  }
  function renderDialog(){
    $('.gp-dialog-layer').hidden=!dialog;const d=$('.gp-dialog');if(!dialog){d.replaceChildren();$('.gp-window').style.minHeight='';return;}
    if(dialog==='close')d.innerHTML=dialogHead('保存这次修改？')+'<p class="gp-dialog-description">原文件和已复制的截图不受影响。</p><div class="gp-dialog-footer"><button type="button" data-action="dismiss">继续编辑</button><button type="button" data-action="confirm-close">放弃更改</button><button type="button" class="gp-primary" data-action="export">保存副本</button></div>';
    if(dialog==='replace')d.innerHTML=dialogHead('打开另一份内容？')+'<p class="gp-dialog-description">当前修改尚未导出。打开新文件会替换当前编辑内容。</p><div class="gp-dialog-footer"><button type="button" data-action="dismiss">继续编辑</button><button type="button" class="gp-primary" data-action="confirm-replace">放弃更改并打开</button></div>';
    if(dialog==='resize')d.innerHTML=dialogHead('调整尺寸')+`<div class="gp-field-row">${number('宽 / px','resize-width',dims()[0],1)}${number('高 / px','resize-height',dims()[1],1)}</div><label class="gp-check"><input type="checkbox" data-field="linked" checked>锁定宽高比</label><div class="gp-dialog-footer"><button type="button" data-action="dismiss">取消</button><button type="button" class="gp-primary" data-action="resize">应用</button></div>`;
    if(dialog==='objects')d.innerHTML=dialogHead('标注对象')+`<div class="gp-object-list">${state().marks.map(m=>`<button type="button" data-object="${m.id}">${icon(m.kind)}<span>${m.kind==='text'?esc(m.text):names[m.kind]}</span>${mode()==='video'?`<small>${Math.round(m.from||0)}–${Math.round(m.to??total())}s</small>`:''}</button>`).join('')||'<span class="gp-dialog-description">还没有标注</span>'}</div>`;
    if(dialog==='export'){
      const video=mode()==='video';d.innerHTML=dialogHead(video?'导出视频':'导出图片')+`<label>文件名<input data-output="name" value="${esc(asset.name.replace(/\.[^.]+$/,''))}-编辑" required></label><div class="gp-field-row"><label>格式<select data-output="format">${(video?['MP4','GIF','MOV','MKV','WebM']:['PNG','JPEG','WebP','BMP','TIFF']).map(v=>`<option>${v}</option>`).join('')}</select></label><label>尺寸<select data-output="size"><option value="original">${dims().join(' × ')}</option><option value="three-quarter">缩小至 75%</option><option value="half">缩小至 50%</option><option value="quarter">缩小至 25%</option></select></label></div>${video?'<label data-codec-row>视频编码<select data-output="codec"><option>H.264 / AVC · 兼容性优先</option><option>H.265 / HEVC · 本机检测后可用</option></select></label>':''}<label data-quality-row ${video?'':'hidden'}><span data-quality-label>${video?'视频质量':'JPEG 质量'}</span><select data-output="quality"><option>高质量</option><option>均衡</option><option>较小文件</option></select></label><label data-fps-row ${video?'':'hidden'}>帧率<select data-output="fps"><option>跟随源文件</option><option>60 fps</option><option>30 fps</option><option>24 fps</option><option>15 fps</option><option>10 fps</option></select></label>${video?'<label data-audio-row>音频编码<select data-output="audio"><option>AAC · 192 kbps</option><option>AAC · 128 kbps</option><option>AAC · 256 kbps</option><option>无声音</option></select></label>'+pair('成片时长',`${total().toFixed(1)} 秒`):''}<p class="gp-dialog-description" data-format-note>重新生成像素，不保留源文件元数据。${video?'H.265 需本机编码器可用；GIF 无声音，最高 30 FPS。':'WebP 使用无损编码；TIFF 仅编辑首页，ICO 读取为单张图片。'}</p><div class="gp-dialog-footer"><button type="button" data-action="dismiss">取消</button><button type="button" class="gp-primary" data-action="confirm-export">导出副本</button></div>`;
    }
    if(dialog==='export'){
      if(mode()==='video'){
        $('[data-output=quality]').selectedIndex=1;
        if(state().muted){$('[data-output=audio]').innerHTML='<option>无声音</option>';$('[data-output=audio]').disabled=true;}
      }else $('[data-output=quality]').outerHTML='<input type="number" data-output="quality" min="1" max="100" value="90" aria-label="JPEG 质量">';
    }
    const windowBox=$('.gp-window').getBoundingClientRect(),height=d.getBoundingClientRect().height+85;if(height>windowBox.height)$('.gp-window').style.minHeight=`${height}px`;
  }
  function openDialog(id,trigger){stop();menu=false;renderMenu();dialog=id;returnFocus=trigger||document.activeElement;$$('.gp-editor-body,.gp-titlebar,.gp-commandbar,.gp-context,.gp-video').forEach(n=>n.inert=true);renderDialog();symbols();$('.gp-dialog button,.gp-dialog input')?.focus();}
  function dismiss(){const replacing=dialog==='replace';dialog='';if(replacing&&pendingAssetId){const i=assets.findIndex(a=>a.id===pendingAssetId);if(i>=0){URL.revokeObjectURL(assets[i].url);assets.splice(i,1);}pendingAssetId=null;}$$('.gp-editor-body,.gp-titlebar,.gp-commandbar,.gp-context,.gp-video').forEach(n=>n.inert=false);renderDialog();if(returnFocus?.isConnected)returnFocus.focus();}
  function selectObject(id){if(dialog)dismiss();selected=id;tool='select';category='select';compare=false;render();$('.gp-context button')?.focus({preventScroll:true});persist();}
  function closeEditor(){stop();textEntry=null;asset.s=clone(asset.baseline);asset.history=[];asset.redo=[];asset=assets[0];stage='result';resultVisible=false;selected=null;category='select';tool='select';render();persist();}
  function historyAction(action){stop();const from=asset[action==='undo'?'history':'redo'],to=asset[action==='undo'?'redo':'history'];if(!from.length)return;to.push(clone(state()));asset.s=from.pop();selected=null;selectedClip=state().clips[0].id;category=mode()==='video'?'clip':'select';tool='select';asset.time=clamp(asset.time,0,total());render();persist();}
  function split(){
    let offset=0;for(let i=0;i<state().clips.length;i++){const c=state().clips[i],len=c.end-c.start;if(asset.time>offset+.1&&asset.time<offset+len-.1){change(()=>{const cut=c.start+asset.time-offset,id=nextId++;state().clips.splice(i,1,{...c,end:cut},{id,start:cut,end:c.end});selectedClip=id;category='clip';selected=null;tool='select';});return;}offset+=len;}
  }
  // Remove a segment and ripple annotation timing through the same output interval.
  function removeClip(){if(state().clips.length<2)return;let at=0;const index=state().clips.findIndex(c=>c.id===selectedClip);if(index<0)return;for(let i=0;i<index;i++)at+=state().clips[i].end-state().clips[i].start;const c=state().clips[index],length=c.end-c.start;
    change(()=>{state().clips.splice(index,1);const remap=t=>t<=at?t:t>=at+length?t-length:at;state().marks=state().marks.map(m=>({...m,from:remap(m.from||0),to:remap(m.to??(total()+length))})).filter(m=>m.to-m.from>.05);asset.time=Math.min(at,total());selectedClip=state().clips[Math.min(index,state().clips.length-1)].id;selected=null;category='clip';});}
  function applyCrop(){
    const c=cropDraft,s=state(),old=s.crop;
    const inverse=(x,y)=>{let u=x,v=y;if(s.rotation===90)[u,v]=[y,1-x];if(s.rotation===180)[u,v]=[1-x,1-y];if(s.rotation===270)[u,v]=[1-y,x];if(s.flip)u=1-u;return [old.x+u*old.w,old.y+v*old.h];};
    const points=[[c.x,c.y],[c.x+c.w,c.y],[c.x,c.y+c.h],[c.x+c.w,c.y+c.h]].map(([x,y])=>inverse(x,y)),xs=points.map(p=>p[0]),ys=points.map(p=>p[1]);
    checkpoint(cropBefore||state());s.crop={x:Math.min(...xs),y:Math.min(...ys),w:Math.max(...xs)-Math.min(...xs),h:Math.max(...ys)-Math.min(...ys)};const[w,h]=dims(),nw=Math.round(w*c.w),nh=Math.round(h*c.h);[s.width,s.height]=s.rotation%180?[nh,nw]:[nw,nh];cropBefore=null;category='select';tool='select';render();persist();
  }
  root.addEventListener('click',event=>{
    const b=event.target.closest('button');if(!b||b.disabled)return;
    if(b.dataset.preview){dismiss();if(b.dataset.preview==='result'){stop();asset=assets[0];stage='result';resultVisible=false;render();persist();}else switchAsset(b.dataset.preview);return;}
    if(b.dataset.category){choose(b.dataset.category);return;}
    if(b.dataset.tool){tool=b.dataset.tool;renderProperties();symbols();return;}
    if(b.dataset.object){selectObject(Number(b.dataset.object));return;}
    if(b.dataset.timedObject){asset.time=state().marks.find(m=>m.id===Number(b.dataset.timedObject)).from||0;selectObject(Number(b.dataset.timedObject));return;}
    if(b.dataset.clip){selectedClip=Number(b.dataset.clip);let at=0;for(const c of state().clips){if(c.id===selectedClip){asset.time=at;break;}at+=c.end-c.start;}category='clip';tool='select';selected=null;render();return;}
    if(b.dataset.color){color=b.dataset.color;if(textEntry){textEntry.m.color=color;renderTextEntry();renderProperties();symbols();}else if(mark())change(()=>mark().color=color);else{renderProperties();symbols();}return;}
    const a=b.dataset.action;
    if(a==='edit-result'){switchAsset(asset.id);return;}
    if(a==='dismiss-result'){resultVisible=false;render();persist();return;}
    if(a==='more'){menu=!menu;renderMenu();symbols();if(menu)$('.gp-more-menu button')?.focus();return;}
    if(a==='open-file'){menu=false;renderMenu();$('.gp-file-input').click();return;}
    if(a==='objects'||a==='resize-dialog'){openDialog(a==='objects'?'objects':'resize',b);return;}
    if(a==='export'){if(textEntry)commitText();openDialog(a,b);return;}
    if(a==='close'){if(textEntry)commitText();cancelCropDraft();if(dirty())openDialog('close',b);else closeEditor();return;}
    if(a==='dismiss'){dismiss();return;}
    if(a==='confirm-close'){dismiss();closeEditor();return;}
    if(a==='confirm-replace'){const id=pendingAssetId;pendingAssetId=null;dismiss();asset.s=clone(asset.baseline);asset.history=[];asset.redo=[];switchAsset(id);return;}
    if(a==='confirm-export'){if(!$('[data-output=name]').reportValidity())return;const format=$('[data-output=format]').value;dismiss();notify(`导出 ${format} 的交互预览；尚未生成文件。`);return;}
    if(a==='copy'){notify('复制操作预览；尚未写入系统剪贴板。');return;}
    if(a==='undo'||a==='redo'){historyAction(a);return;}
    if(a==='compare')return;
    if(a==='edit-text'&&mark()?.kind==='text'){beginText(mark());return;}
    if(a==='commit-text'){commitText();return;}
    if(a==='cancel-text'){cancelText();return;}
    if(a==='fit'){zoom=1;pan={x:0,y:0};renderImage();persist();return;}
    if(['zoom-in','zoom-out','zoom-actual'].includes(a)){zoom=a==='zoom-actual'?dims()[0]/Math.max(1,$('.gp-image-frame').offsetWidth):clamp(zoom*(a==='zoom-in'?1.25:.8),.25,8);renderImage();persist();return;}
    if(a==='rotate'||a==='flip'){const transform=()=>{if(a==='rotate')state().rotation=(state().rotation+90)%360;else state().flip=!state().flip;};if(category==='crop'){transform();if(cropRatio!=='free'){const[x,y]=cropRatio.split(':').map(Number),[w,h]=dims(),r=x/y*h/w;cropDraft.w=Math.min(.84,.8*r);cropDraft.h=cropDraft.w/r;cropDraft.x=(1-cropDraft.w)/2;cropDraft.y=(1-cropDraft.h)/2;}render();}else change(transform);return;}
    if(a==='resize'){const w=$('[data-field=resize-width]'),h=$('[data-field=resize-height]');if(!w.reportValidity()||!h.reportValidity())return;const values=state().rotation%180?[Number(h.value),Number(w.value)]:[Number(w.value),Number(h.value)];dismiss();change(()=>{[state().width,state().height]=values;});return;}
    if(a==='apply-crop'){applyCrop();return;}
    if(a==='cancel-crop'){choose('select');return;}
    if(a==='reset-adjust'){change(()=>Object.assign(state(),{brightness:100,contrast:100,saturation:100}));return;}
    if(a==='delete'&&mark()){change(()=>{state().marks=state().marks.filter(m=>m.id!==selected);selected=null;});return;}
    if(a==='duplicate'&&mark()){change(()=>{const m=clone(mark());m.id=nextId++;m.x+=24;m.y+=24;if(m.points)m.points=m.points.map(([x,y])=>[x+24,y+24]);state().marks.push(m);selected=m.id;});return;}
    if(a==='split'){split();return;}
    if(a==='delete-clip'){removeClip();return;}
    if(a==='audio'){choose('audio');return;}
    if(a==='frame-next'||a==='frame-back'){stop();asset.time=clamp(asset.time+(a==='frame-next'?1:-1)/30,0,total());renderTime();return;}
    if(a==='play'){if(playing){stop();renderTime();}else{playing=true;if(asset.time>=total())asset.time=0;timer=setInterval(()=>{asset.time=Math.min(total(),asset.time+.1);if(asset.time>=total())stop();renderTime();},100);renderTime();}return;}
  });
  root.addEventListener('focusin',e=>{if(e.target.matches('input[type=range]:not(.gp-seek)'))rangeBefore=clone(state());});
  root.addEventListener('input',e=>{
    const n=e.target,k=n.dataset.field;
    if(k==='seek'){stop();asset.time=Number(n.value);renderTime();return;}
    if(['brightness','contrast','saturation','volume'].includes(k)){rangeBefore||=clone(state());state()[k]=Number(n.value);$(`[data-value=${k}]`).textContent=`${n.value}%`;renderImage();return;}
    if(k==='resize-width'||k==='resize-height'){if($('[data-field=linked]').checked){const [w,h]=dims(),other=k==='resize-width'?'resize-height':'resize-width';$(`[data-field=${other}]`).value=Math.round(Number(n.value)*(other==='resize-height'?h/w:w/h));}}
  });
  root.addEventListener('change',e=>{
    const n=e.target,k=n.dataset.field;if(n.matches('input[type=number]')&&!n.reportValidity()){renderProperties();symbols();return;}
    if(n.dataset.output==='format'){
      const gif=n.value==='GIF',webm=n.value==='WebM',video=mode()==='video';
      $('[data-quality-row]').hidden=video?gif:n.value!=='JPEG';$('[data-quality-label]').textContent=video?'视频质量':'JPEG 质量';$('[data-fps-row]').hidden=!video;
      if(video){
        $('[data-codec-row]').hidden=gif;$('[data-audio-row]').hidden=gif;
        $('[data-output=codec]').innerHTML=(webm?['VP9']:['H.264 / AVC · 兼容性优先','H.265 / HEVC · 本机检测后可用']).map(v=>`<option>${v}</option>`).join('');
        $('[data-output=audio]').innerHTML=(state().muted?['无声音']:webm?['Opus · 192 kbps','Opus · 128 kbps','Opus · 256 kbps','无声音']:['AAC · 192 kbps','AAC · 128 kbps','AAC · 256 kbps','无声音']).map(v=>`<option>${v}</option>`).join('');
        $('[data-output=fps]').innerHTML=(gif?['跟随源文件','30 fps','24 fps','15 fps','10 fps']:['跟随源文件','60 fps','30 fps','24 fps','15 fps','10 fps']).map(v=>`<option>${v}</option>`).join('');
        $('[data-format-note]').textContent=gif?'GIF 无声音，最高 30 FPS，且不高于源文件。':webm?'WebM 使用 VP9 + Opus；需要本地视频格式组件。':n.value==='MP4'?'H.265 需本机编码器可用；帧率不高于源文件，尺寸会补齐至 16 像素倍数。':'使用 H.264 / H.265 + AAC；需要本地视频格式组件，尺寸会补齐至 16 像素倍数。';
      }return;
    }
    if(['brightness','contrast','saturation','volume'].includes(k)){if(rangeBefore){checkpoint(rangeBefore);rangeBefore=null;render();persist();}return;}
    if(k==='muted'){change(()=>state().muted=n.checked);return;}
    if(k==='crop-ratio'){cropRatio=n.value;const[w,h]=dims();if(cropRatio!=='free'){const[a,b]=cropRatio.split(':').map(Number),ratio=a/b*h/w;cropDraft.w=Math.min(.84,.8*ratio);cropDraft.h=cropDraft.w/ratio;cropDraft.x=(1-cropDraft.w)/2;cropDraft.y=(1-cropDraft.h)/2;}renderProperties();renderCrop();symbols();return;}
    if(k==='crop-w'||k==='crop-h'){const dim=k==='crop-w'?'w':'h',axis=dim==='w'?'x':'y',size=dims()[dim==='w'?0:1];cropDraft[dim]=clamp(Number(n.value)/size,.02,1-cropDraft[axis]);cropRatio='free';renderProperties();renderCrop();symbols();return;}
    if(k==='clip-start'||k==='clip-end'){
      const c=state().clips.find(c=>c.id===selectedClip),start=k==='clip-start'?Number(n.value):c.start,end=k==='clip-end'?Number(n.value):c.end;if(end-start<.1){notify('片段至少保留 0.1 秒。');renderProperties();return;}
      change(()=>{const oldTotal=total();c.start=start;c.end=end;const newTotal=total();state().marks=state().marks.map(m=>({...m,to:Math.min(m.to??oldTotal,newTotal)})).filter(m=>(m.to??newTotal)>(m.from||0));});return;
    }
    if(k==='text'){text=n.value;if(mark())change(()=>{mark().text=text;mark().w=Math.max(20,text.length*(mark().font||40)*.9);});return;}
    if(k==='font'||k==='line'||k==='weight'){const value=Number(n.value);if(k==='font')font=value;if(k==='line')line=value;if(textEntry){textEntry.m[k]=value;renderTextEntry();}else if(mark())change(()=>{mark()[k]=value;if(k==='font'){mark().h=value*1.2;mark().w=mark().text.length*value*.9;}});return;}
    if(k?.startsWith('mark-')&&mark()){
      const key=k.slice(5),value=Number(n.value),m=mark();if(key==='from'&&value>=(m.to??total())||key==='to'&&value<=(m.from||0)){notify('结束时间需要晚于出现时间。');renderProperties();return;}
      change(()=>{if(['w','h'].includes(key)&&m.points){const old=m[key]||1;m.points=m.points.map(([x,y])=>key==='w'?[m.x+(x-m.x)*value/old,y]:[x,m.y+(y-m.y)*value/old]);}if(['x','y'].includes(key)&&m.points)m.points=m.points.map(([x,y])=>key==='x'?[x+value-m.x,y]:[x,y+value-m.y]);m[key]=value;});return;
    }
  });
  const svg=$('.gp-annotations');
  function point(e){const p=svg.createSVGPoint();p.x=e.clientX;p.y=e.clientY;return p.matrixTransform(svg.getScreenCTM().inverse());}
  svg.addEventListener('pointerdown',e=>{
    if(textEntry){commitText();return;}
    if(e.button!==0||category==='crop'||compare||dialog||space)return;const p=point(e),before=clone(state());svg.setPointerCapture(e.pointerId);
    if(tool==='select'){
      const handle=e.target.dataset.handle,target=e.target.closest('[data-mark]');if(!handle)selected=target?Number(target.dataset.mark):null;
      const m=mark();drag=m?{type:handle?'resize':'move',handle,p,before,original:clone(m),moved:false}:null;renderProperties();renderMarks();symbols();return;
    }
    const m={id:nextId++,kind:tool,x:p.x,y:p.y,w:0,h:0,color,line,from:asset.time,to:Math.min(total(),asset.time+5)};
    if(tool==='text'){m.text=text;m.font=font;m.weight=500;m.w=text.length*font*.9;m.h=font*1.2;beginText(m,true);return;}
    if(tool==='pen')m.points=[[p.x,p.y]];state().marks.push(m);drag={type:'draw',p,id:m.id,before};renderMarks();
  });
  svg.addEventListener('pointermove',e=>{
    if(!drag)return;const p=point(e),dx=p.x-drag.p.x,dy=p.y-drag.p.y;
    if(drag.type==='move'){const m=mark(),o=drag.original;m.x=o.x+dx;m.y=o.y+dy;if(m.points)m.points=o.points.map(([x,y])=>[x+dx,y+dy]);drag.moved=Math.abs(dx)+Math.abs(dy)>1;}
    else if(drag.type==='resize'){
      const m=mark(),o=drag.original,b=bounds(o),h=drag.handle;let x=b.x,y=b.y,w=b.w,he=b.h;
      if(h.includes('w')){x=Math.min(b.x+dx,b.x+b.w-8);w=b.x+b.w-x;}else w=Math.max(8,b.w+dx);
      if(h.includes('n')){y=Math.min(b.y+dy,b.y+b.h-8);he=b.y+b.h-y;}else he=Math.max(8,b.h+dy);
      m.x=x;m.y=y;m.w=w;m.h=he;if(m.kind==='arrow'){if(o.w<0){m.x=x+w;m.w=-w;}if(o.h<0){m.y=y+he;m.h=-he;}}if(o.points)m.points=o.points.map(([px,py])=>[x+(px-b.x)*w/Math.max(1,b.w),y+(py-b.y)*he/Math.max(1,b.h)]);if(m.kind==='text')m.font=Math.max(12,o.font*he/Math.max(1,b.h));drag.moved=true;
    }else{const m=state().marks.find(m=>m.id===drag.id);m.w=dx;m.h=dy;if(m.kind==='pen'){m.points.push([p.x,p.y]);const xs=m.points.map(p=>p[0]),ys=m.points.map(p=>p[1]);m.x=Math.min(...xs);m.y=Math.min(...ys);m.w=Math.max(...xs)-m.x;m.h=Math.max(...ys)-m.y;}else if(m.kind!=='arrow'){m.x=Math.min(drag.p.x,p.x);m.y=Math.min(drag.p.y,p.y);m.w=Math.abs(dx);m.h=Math.abs(dy);}}
    renderMarks();
  });
  function finish(cancel=false){if(!drag)return;if(cancel)asset.s=drag.before;else if(drag.type==='draw'){const m=state().marks.find(m=>m.id===drag.id);if(Math.abs(m.w)+Math.abs(m.h)<5)state().marks.pop();else{checkpoint(drag.before);selected=m.id;tool='select';category='select';}}else if(drag.moved)checkpoint(drag.before);drag=null;render();persist();}
  svg.addEventListener('pointerup',()=>finish());svg.addEventListener('pointercancel',()=>finish(true));
  svg.addEventListener('dblclick',e=>{const g=e.target.closest('[data-mark]'),m=g?state().marks.find(m=>m.id===Number(g.dataset.mark)):mark(),p=point(e);if(m?.kind==='text'&&p.x>=m.x&&p.x<=m.x+m.w&&p.y>=m.y&&p.y<=m.y+m.h)beginText(m);});
  let cropDrag=null;
  $('.gp-crop').addEventListener('pointerdown',e=>{if(!e.target.closest('[data-crop-handle]'))return;const box=$('.gp-image-frame').getBoundingClientRect();cropDrag={handle:e.target.closest('[data-crop-handle]').dataset.cropHandle,x:e.clientX,y:e.clientY,box,before:{...cropDraft}};$('.gp-crop').setPointerCapture(e.pointerId);});
  $('.gp-crop').addEventListener('pointermove',e=>{
    if(!cropDrag)return;const d=cropDrag,o=d.before,dx=(e.clientX-d.x)/d.box.width,dy=(e.clientY-d.y)/d.box.height;
    if(d.handle==='move'){cropDraft.x=clamp(o.x+dx,0,1-o.w);cropDraft.y=clamp(o.y+dy,0,1-o.h);}
    else{let left=o.x,top=o.y,right=o.x+o.w,bottom=o.y+o.h;if(d.handle.includes('w'))left=clamp(o.x+dx,0,right-.04);else right=clamp(right+dx,left+.04,1);if(d.handle.includes('n'))top=clamp(o.y+dy,0,bottom-.04);else bottom=clamp(bottom+dy,top+.04,1);let w=right-left,h=bottom-top;
      if(cropRatio!=='free'){const[a,b]=cropRatio.split(':').map(Number),[iw,ih]=dims(),r=a/b*ih/iw;w=Math.min(w,h*r);h=w/r;if(d.handle.includes('w'))left=right-w;if(d.handle.includes('n'))top=bottom-h;}cropDraft={x:left,y:top,w,h};}
    renderCrop();
  });
  $('.gp-crop').addEventListener('pointerup',()=>cropDrag=null);$('.gp-crop').addEventListener('pointercancel',()=>{if(cropDrag)cropDraft=cropDrag.before;cropDrag=null;renderCrop();});
  let panDrag=null;const canvas=$('.gp-stage');
  canvas.addEventListener('pointerdown',e=>{if((space||e.button===1)&&!dialog){e.preventDefault();e.stopImmediatePropagation();panDrag={x:e.clientX,y:e.clientY,before:{...pan}};canvas.setPointerCapture(e.pointerId);}},true);
  canvas.addEventListener('pointermove',e=>{if(panDrag){pan={x:panDrag.before.x+e.clientX-panDrag.x,y:panDrag.before.y+e.clientY-panDrag.y};renderImage();}});
  canvas.addEventListener('pointerup',()=>panDrag=null);canvas.addEventListener('pointercancel',()=>{if(panDrag)pan=panDrag.before;panDrag=null;renderImage();});
  canvas.addEventListener('wheel',e=>{if(!dialog&&!textEntry){e.preventDefault();zoom=clamp(zoom*(e.deltaY<0?1.1:.9),.25,8);renderImage();}}, {passive:false});
  function setComparison(value){if(textEntry)return;compare=value;$('[data-action=compare]').setAttribute('aria-pressed',String(value));renderImage();}
  const compareButton=$('[data-action=compare]');compareButton.addEventListener('pointerdown',e=>{if(e.button!==0)return;compareButton.setPointerCapture(e.pointerId);setComparison(true);});compareButton.addEventListener('pointerup',()=>setComparison(false));compareButton.addEventListener('pointercancel',()=>setComparison(false));compareButton.addEventListener('lostpointercapture',()=>setComparison(false));compareButton.addEventListener('blur',()=>setComparison(false));
  document.addEventListener('pointerdown',e=>{if(menu&&!e.target.closest('.gp-more-menu,[data-action=more]')){menu=false;renderMenu();}});
  root.addEventListener('keydown',e=>{
    if(textEntry&&e.target.closest('.gp-text-entry')){if(e.key==='Escape'){e.preventDefault();cancelText();}else if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();commitText();$('.gp-toolrail [data-category=select]')?.focus({preventScroll:true});}return;}
    if(e.target.closest('[data-action=compare]')&&[' ','Enter'].includes(e.key)){e.preventDefault();setComparison(true);return;}
    if(menu){if(e.key==='Escape'){e.preventDefault();menu=false;renderMenu();$('[data-action=more]').focus();return;}if(['ArrowDown','ArrowUp'].includes(e.key)){e.preventDefault();const buttons=$$('.gp-more-menu button'),index=buttons.indexOf(document.activeElement);buttons[(index+(e.key==='ArrowDown'?1:buttons.length-1))%buttons.length].focus();return;}}
    if(dialog){if(e.key==='Escape'){e.preventDefault();dismiss();}if(e.key==='Tab'){const items=[...$('.gp-dialog').querySelectorAll('button,input,select')].filter(n=>!n.disabled&&!n.closest('[hidden]'));if(e.shiftKey&&document.activeElement===items[0]){e.preventDefault();items.at(-1).focus();}else if(!e.shiftKey&&document.activeElement===items.at(-1)){e.preventDefault();items[0].focus();}}return;}
    if(e.key==='Escape'){e.preventDefault();if(stage==='result'){resultVisible=false;render();}else if(category!=='select'||selected!==null){selected=null;choose('select');}else if(dirty())openDialog('close');else closeEditor();return;}
    if(e.target.matches('input,textarea,select'))return;
    if(e.key===' '){e.preventDefault();if(mode()==='video'){if(!e.repeat)$('[data-action=play]').click();}else space=true;return;}
    if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='z'){e.preventDefault();historyAction(e.shiftKey?'redo':'undo');return;}
    if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='c'&&mode()==='image'){e.preventDefault();$('[data-action=copy]').click();return;}
    if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'){e.preventDefault();openDialog('export');return;}
    if(e.key==='Delete'&&selected!==null){e.preventDefault();$('[data-action=delete]')?.click();}
    if(e.key==='Enter'&&category==='crop'){e.preventDefault();applyCrop();}
    if(mode()==='video'&&['ArrowLeft','ArrowRight'].includes(e.key)){e.preventDefault();stop();asset.time=clamp(asset.time+(e.key==='ArrowRight'?1:-1)/30,0,total());renderTime();}
  });
  root.addEventListener('keyup',e=>{if([' ','Enter'].includes(e.key)&&e.target.closest('[data-action=compare]'))setComparison(false);if(e.key===' ')space=false;});window.addEventListener('blur',()=>{space=false;setComparison(false);});
  const entry=new URLSearchParams(location.search);
  if(entry.has('open')) { stage='editor'; render(); $('.gp-file-input').click(); }
  else if(['image','video'].includes(entry.get('editor'))) switchAsset(entry.get('editor'));
  $('.gp-file-input').addEventListener('change',e=>{
    const file=e.target.files?.[0];e.target.value='';if(!file)return;const kind=file.type.startsWith('image/')?'image':file.type.startsWith('video/')?'video':null;
    if(!kind){notify('请选择图片或 MP4、MOV、M4V、WMV、GIF 文件；实际读取能力以本机解码器为准。');return;}
    const url=URL.createObjectURL(file),probe=document.createElement(kind==='image'?'img':'video');let done=false;
    const fail=()=>{if(done)return;done=true;URL.revokeObjectURL(url);notify('无法读取这个文件；当前编辑已保留。');};
    const loaded=()=>{if(done)return;done=true;const w=kind==='image'?probe.naturalWidth:probe.videoWidth,h=kind==='image'?probe.naturalHeight:probe.videoHeight,d=kind==='video'?probe.duration:0;if(!w||!h||kind==='video'&&(!Number.isFinite(d)||d<=0)){URL.revokeObjectURL(url);notify('文件尺寸或时长无效。');return;}
      const id=`local-${nextId++}`,s=initial(kind,w,h,d||32);s.marks=[];assets.push({id,kind,name:file.name,w,h,duration:d,url,s,baseline:clone(s),history:[],redo:[],time:0});if(dirty()){pendingAssetId=id;openDialog('replace');}else switchAsset(id);};
    probe.addEventListener(kind==='image'?'load':'loadedmetadata',loaded,{once:true});probe.addEventListener('error',fail,{once:true});probe.src=url;
  });
  function restore(saved){const m=saved?.modelContent;if(m?.design!=='Glint workbench v7')return;if(['system','light','dark'].includes(m.appearance))design.appearance=m.appearance;if(['image','video'].includes(m.mode))asset=assets.find(a=>a.id===saved.privateContent?.assetId)||assets.find(a=>a.id===m.mode);if(['editor','result'].includes(m.stage))stage=m.stage;if(['select','crop','shape','text','redact','adjust','clip','audio'].includes(m.tool))category=m.tool;if(Object.keys(names).includes(saved.privateContent?.activeTool))tool=saved.privateContent.activeTool;if(typeof saved.privateContent?.resultVisible==='boolean')resultVisible=saved.privateContent.resultVisible;zoom=clamp(Number(saved.privateContent?.zoom)||1,.25,8);render();}
  const sampleMarkup=$('.gp-sample').innerHTML;$('.gp-desktop-content').innerHTML=sampleMarkup;$('.gp-result-thumbnail').innerHTML=sampleMarkup;
  render();restore(window.openai?.widgetState);new ResizeObserver(()=>renderImage()).observe($('.gp-stage'));
  window.addEventListener('openai:set_globals',e=>{if(e.detail?.globals?.widgetState)restore(e.detail.globals.widgetState);});
  window.addEventListener('pagehide',()=>{stop();assets.forEach(a=>{if(a.url)URL.revokeObjectURL(a.url);});});
  if(globalThis.Tweak){const tweak=new Tweak({container:root,onChange:render});tweak.addSelect(design,'appearance',{label:'窗口主题',options:[{label:'跟随系统',value:'system'},{label:'浅色',value:'light'},{label:'深色',value:'dark'}]});tweak.addColorPicker(design,'accent',{label:'强调色',reference:'--gp-accent'});}
})();
