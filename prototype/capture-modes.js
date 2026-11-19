// Capture interaction specification. Illustrative pixels only; no OS capture.
// Live-capture contract: retain the selection boundary and show the adjacent
// preview/control windows immediately. A pending first frame has an explicit
// loading state; startup failure restores the frozen selection for retry.
// A stable source permits the next automatic scroll even if a join is pending.
// Pending joins are retained for completion/recovery, never a stability gate.
// Keep the boundary and adjacent controls visible to desktop sharing as well.
// Exclude only a controller that actually overlaps the source pixels.
function modeButton(action,icon,label,{disabled=false,primary=false}={}){
 return `<button data-action="${action}" title="${label}" aria-label="${label}" ${disabled?'disabled':''} ${primary?'class="primary"':''}>${toolbarIcon(icon)}</button>`;
}
function captureModeBar(){if(state.localImage||state.editingPin||state.colorOnly)return '';return `<nav class="capture-mode-bar" aria-label="截图模式">${[['capture','截图'],['scroll','滚动长截图'],['record','录屏']].map(([mode,label])=>`<button data-mode="${mode}" class="${(state.captureMode||'capture')===mode?'active':''}">${label}</button>`).join('')}</nav>`;}
function captureModeControls(){
 const box=sourceBox(), pageTop=$('.desktop')?.getBoundingClientRect().top||56;
 const recording=state.captureMode==='record';
 const height=recording?90:46, width=Math.min(recording?418:232,innerWidth-24);
 const left=Math.max(12,Math.min(innerWidth-width-12,box.x+(box.w-width)/2));
 const below=box.y+box.h+12;
 const top=(below+height+12<=innerHeight?below:Math.max(pageTop+70,box.y-height-12))-pageTop;
 const position=`left:${left}px;top:${top}px;width:${width}px`;
 if(!recording){
   return `<section class="capture-mode-setup scroll-setup" style="${position}" aria-label="长截图">
     <div class="mode-actions"><button data-action="cancel-capture">取消</button><button class="primary" data-action="scroll-plan">开始滚动截图</button></div>
   </section>`;
 }
 return `<section class="capture-mode-setup record-setup" style="${position}" aria-label="录屏选项">
   <div class="mode-options">
     <select id="record-format" aria-label="格式">${['MP4','GIF'].map(format=>`<option ${saved.recordFormat===format?'selected':''}>${format}</option>`).join('')}</select>
     <label class="mode-toggle"><input id="record-cursor" type="checkbox" ${saved.recordCursor?'checked':''}>鼠标指针</label>
     <label class="mode-toggle"><input id="record-audio" type="checkbox" ${saved.recordAudio?'checked':''} ${saved.recordFormat==='GIF'?'disabled':''}>系统声音</label>
     <label class="mode-toggle"><input id="record-mic" type="checkbox" ${saved.recordMic?'checked':''} ${saved.recordFormat==='GIF'?'disabled':''}>麦克风</label>
   </div>
   <div class="mode-actions"><button data-action="cancel-capture">取消</button><button class="primary" data-action="record-plan">开始录制</button></div>
 </section>`;
}
function sourceBox(){const r=state.selection||{x:160,y:140,w:660,h:400},b=$('#capture-surface')?.getBoundingClientRect();return {x:r.x+(b?.x||0),y:r.y+(b?.y||56),w:r.w,h:r.h};}
function longSample(count=1){return `<div class="long-document"><header><small>Glint · 原型示意</small><h2>项目进展记录</h2><p>内容仅用于交互演示，不读取真实屏幕。</p></header>${Array.from({length:count},(_,i)=>`<article><h3>${i+1}. ${['本周工作','方案与进度','数据回顾','下一步计划'][i%4]}</h3><p>保留重要信息，让内容自然连接。</p><div class="document-chart"><i></i><i></i><i></i><i></i></div><p>滚动查看后续内容，右侧同步展示拼接效果。</p><div class="document-lines"></div></article>`).join('')}</div>`;}
function liveDockLeft(box,width){
 return Math.max(8,Math.min(innerWidth-width-8,box.x+(box.w-width)/2));
}
function liveDockPosition(box,width,height=44){
 width=Math.min(width,innerWidth-16);
 const left=liveDockLeft(box,width);
 const below=box.y+box.h+8, above=box.y-height-8;
 const top=below+height<=innerHeight-8?below:Math.max(8,above);
 return `left:${left}px;top:${top}px;width:${width}px`;
}
function liveHole(box,content=''){
 return `<div class="live-capture-hole" style="left:${box.x}px;top:${box.y}px;width:${box.w}px;height:${box.h}px">${content}</div>`;
}
function renderScrollSession(session){
 const box=sourceBox(), dockWidth=Math.min(384,innerWidth-16);
 const selectionRight=box.x+box.w+12;
 const clearOfDock=Math.max(selectionRight,liveDockLeft(box,dockWidth)+dockWidth+8);
 const previewLeft=innerWidth-clearOfDock-8>=120?clearOfDock:selectionRight;
 const right=innerWidth-previewLeft-8;
 const width=Math.min(innerWidth-16,280,Math.max(120,right));
 const x=Math.min(innerWidth-width-8,previewLeft);
 const top=Math.max(8,Math.min(box.y,innerHeight-96));
 const scale=width/box.w;
 const capturedHeight=box.h+Math.max(0,...(session.offsets||[]));
 const height=Math.min(innerHeight-top-12,Math.ceil(capturedHeight*scale));
 const partial=session.stopped&&session.pending;
 const overlaps=x<box.x+box.w&&x+width>box.x;
 return `<div class="live-session scroll-session" aria-label="滚动截图">
   ${liveHole(box,`<div class="scroll-source" tabindex="0" aria-label="滚动采集区域">${longSample(12)}</div>`)}
   <aside class="stitch-preview ${overlaps&&!session.stopped?'preview-pass-through':''}" style="left:${x}px;top:${top}px;width:${width}px" aria-label="拼接预览">
     <div class="stitch-image" style="height:${height}px">
       <div class="stitch-scaled" style="height:${capturedHeight*scale}px">
         <div style="width:${box.w}px;height:${capturedHeight}px;transform:scale(${scale})">${longSample(12)}</div>
       </div>
     </div>
     ${session.notice?`<div class="scroll-notice" role="status">${session.notice}</div>`:''}
   </aside>
   <div class="scroll-controls" style="${liveDockPosition(box,dockWidth,partial?86:44)}">
   <div class="toolbar live-toolbar scroll-dock" role="toolbar" aria-label="长截图">
     <button class="live-labeled" data-action="scroll-auto" ${session.stopped?'disabled':''} aria-pressed="${!!session.auto}">${session.auto?'停止自动':'自动滚动'}</button>
     <button class="live-labeled" data-action="scroll-stop">${session.stopped?'继续采集':'结束采集'}</button>
     ${modeButton('scroll-undo','undo','撤销上次拼接',{disabled:session.count<=1&&!session.pending})}
     <span class="divider"></span>
     ${modeButton('scroll-edit','pen',partial?'编辑已拼接部分':'编辑长图')}
     ${modeButton('scroll-save','save',partial?'保存已拼接部分':'保存长图')}
     <button class="primary live-labeled" data-action="scroll-copy" title="${partial?'复制已拼接部分':'复制长图'}">复制</button>
     ${modeButton('scroll-cancel','close','取消长截图')}
   </div>
   ${partial?`<div class="scroll-repair" role="group" aria-label="拼接修复">
     ${session.repair?'<label>重叠 <input type="number" id="scroll-overlap" min="0" max="9999" value="'+session.overlap+'"></label><button data-action="scroll-discard">丢弃</button><button class="primary" data-action="scroll-accept">应用</button>':'<button data-action="scroll-repair">调整拼接</button>'}
   </div>`:''}
   </div>
 </div>`;
}
function recordTime(seconds){
 const total=Math.max(0,Math.floor(seconds||0));
 const hours=Math.floor(total/3600);
 const minutes=String(Math.floor(total/60)%60).padStart(2,'0');
 const remainder=String(total%60).padStart(2,'0');
 return `${hours>0?String(hours).padStart(2,'0')+':':''}${minutes}:${remainder}`;
}
function recordCountdownText(seconds){
 return `即将录制 · ${seconds} 秒`;
}
function renderRecordingSession(){
 const session=state.modal, box=sourceBox(), countdown=session.phase==='countdown', paused=session.phase==='paused';
 if(session.phase==='stopped')return `<div class="record-review-backdrop"><section class="record-review" role="dialog" aria-label="录制结果">
   <header><b>录制结果 · ${session.format}</b><span class="grow"></span>${modeButton('close-modal','close','关闭')}</header>
   <div class="record-result-image">${longSample(1)}</div>
   <footer><time>${recordTime(session.elapsed)}</time><span class="grow"></span><button class="primary" data-action="record-edit">剪辑视频…</button></footer>
 </section></div>`;
 return `<div class="live-session record-session" aria-label="录屏">
   ${liveHole(box,countdown?`<div class="record-countdown" role="status"><span>${session.seconds}</span></div>`:'')}
   <div class="toolbar live-toolbar record-dock" style="${liveDockPosition(box,300)}" role="toolbar" aria-label="录屏控制">
     ${countdown?`<span class="record-start-status">${recordCountdownText(session.seconds)}</span>`:`<span class="record-status"><span class="record-light ${paused?'paused':''}" aria-hidden="true"></span><time class="record-time">${recordTime(session.elapsed)}</time></span>
     <span class="record-separator" aria-hidden="true"></span>
     <button class="live-labeled" data-action="record-demo" data-value="${paused?'active':'paused'}">${paused?'继续':'暂停'}</button>
     <button class="primary live-labeled" data-action="record-demo" data-value="stopped">停止并保存</button>`}
     <button data-action="record-demo" data-value="discard" aria-label="${countdown?'取消倒计时':'放弃录制'}" title="${countdown?'取消倒计时':'放弃录制'}">${toolbarIcon('close')}</button>
   </div>
 </div>`;
}
let prototypeScrollTimer,prototypeRecordTimer,prototypeWheelTimer;
function beginRecordingDemo(){
 const seconds=saved.recordCountdown;
 const format=$('#record-format')?.value||saved.recordFormat;
 const cursor=$('#record-cursor')?.checked??saved.recordCursor;
 const audio=format==='MP4'&&($('#record-audio')?.checked??saved.recordAudio);
 const microphone=format==='MP4'&&($('#record-mic')?.checked??saved.recordMic);
 openModal('recording',{phase:seconds?'countdown':'active',seconds,elapsed:0,format,cursor,audio,microphone});
 setRecordingPhase(state.modal.phase);
}
document.addEventListener('change',event=>{
 if(event.target.id==='scroll-overlap'&&state.modal?.type==='scroll-capture'){
   state.modal.overlap=Math.max(0,Math.min(9999,Math.round(Number(event.target.value)||0)));
   return;
 }
 if(event.target.id!=='record-format')return;
 const gif=event.target.value==='GIF';
 for(const field of ['record-audio','record-mic']){
   const input=document.getElementById(field);
   if(input)input.disabled=gif;
 }
});
function setRecordingPhase(phase){
 const session=state.modal;
 if(session?.type!=='recording')return;
 clearInterval(prototypeRecordTimer);
 if(phase==='discard'){closeModal();return;}
 session.phase=phase;
 renderModal();
 if(phase!=='active'&&phase!=='countdown')return;
 prototypeRecordTimer=setInterval(()=>{
   if(state.modal!==session){clearInterval(prototypeRecordTimer);return;}
   if(session.phase==='countdown'){
     if(--session.seconds===0){setRecordingPhase('active');return;}
     const badge=$('.record-countdown span');
     if(badge)badge.textContent=session.seconds;
     const status=$('.record-start-status');
     if(status)status.textContent=recordCountdownText(session.seconds);
   }else{
     session.elapsed++;
     const clock=$('.record-time');
     if(clock)clock.textContent=recordTime(session.elapsed);
   }
 },1000);
}
function sampleScrollSource(session,offset){
 if(state.modal!==session||session.stopped||offset===session.offsets.at(-1))return;
 session.offsets.push(offset);
 session.count=session.offsets.length;
 session.notice='';
 renderModal();
}
document.addEventListener('scroll',event=>{
 const session=state.modal, source=event.target;
 if(session?.type!=='scroll-capture'||session.stopped||source!==$('.scroll-source'))return;
 const offset=source.scrollTop;
 clearTimeout(prototypeWheelTimer);
 prototypeWheelTimer=setTimeout(()=>sampleScrollSource(session,offset),180);
},true);
function toggleAutoScroll(session){
 session.auto=!session.auto;
 session.notice='';
 clearInterval(prototypeScrollTimer);
 if(session.auto)prototypeScrollTimer=setInterval(()=>{
   if(state.modal!==session||!session.auto||session.stopped){clearInterval(prototypeScrollTimer);return;}
   const source=$('.scroll-source');
   if(!source)return;
   const previous=source.scrollTop;
   const next=Math.min(source.scrollHeight-source.clientHeight,previous+Math.round(source.clientHeight*.55));
   if(next<=previous){
     session.auto=false;
     session.notice='已滚动到底部';
     clearInterval(prototypeScrollTimer);
     renderModal();
     return;
   }
   source.scrollTop=next;
 },440);
 renderModal();
}
document.addEventListener('click',event=>{
 const mode=event.target.closest('[data-mode]')?.dataset.mode;
 if(mode){
   if(state.marks.length){toast('请先撤销标注，再切换采集模式。');return;}
   state.captureMode=mode;
   state.tool='select';
   render();
   return;
 }
 const action=event.target.closest('[data-action]')?.dataset.action;
 const session=state.modal;
 if(action==='close-modal'){
   clearInterval(prototypeScrollTimer);
   clearInterval(prototypeRecordTimer);
   return;
 }
 if(session?.type!=='scroll-capture')return;
 if(action==='scroll-cancel'){
   clearInterval(prototypeScrollTimer);
   clearTimeout(prototypeWheelTimer);
   state.captureMode='capture';
   state.modal=null;
   cancelCapture();
   return;
 }
 if(action==='scroll-repair'){
   session.repair=true;
   renderModal();
   return;
 }
 if(action==='scroll-auto'){
   toggleAutoScroll(session);
   return;
 }
 const output=['scroll-copy','scroll-save','scroll-edit'].includes(action);
 if(action==='scroll-stop'||output&&!session.stopped){
   sampleScrollSource(session,$('.scroll-source').scrollTop);
   session.stopped=!session.stopped;
   if(!session.stopped)session.repair=false;
   session.auto=false;
   session.notice='';
   clearInterval(prototypeScrollTimer);
   clearTimeout(prototypeWheelTimer);
   renderModal();
   if(output&&session.pending){
     event.stopImmediatePropagation();
     toast('部分内容未能拼接，片段已保留。可输出已对齐部分或调整拼接。');
     return;
   }
 }
 if(action==='scroll-save'||action==='scroll-copy')toast('交互演示，不生成文件或修改剪贴板。');
},true);

document.addEventListener('keydown',event=>{if(event.key==='Escape'&&state.modal?.type==='scroll-capture'){event.preventDefault();event.stopImmediatePropagation();clearInterval(prototypeScrollTimer);clearTimeout(prototypeWheelTimer);state.captureMode='capture';state.modal=null;cancelCapture();}},true);
