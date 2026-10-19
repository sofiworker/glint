/* Browser-only editable-object interaction specification. No screenshot pixels. */
'use strict';
function resetEdits(){state.marks=[];state.undoSteps=[];state.redo=[];state.objectSelected=null;}
function checkpoint(before=state.marks){state.undoSteps.push(clone(before));state.redo=[];}
function abortObjectDrag(){if(state.drag?.mode==='object'){state.marks=state.drag.before;state.drag=null;}}
function abortCaptureGesture(){const d=state.drag;if(d?.selectionBefore){state.selection=d.original;state.marks=d.selectionBefore.marks;state.undoSteps=d.selectionBefore.undo;state.redo=d.selectionBefore.redo;}abortObjectDrag();state.drag=null;state.draftMark=null;}
function captureGesturePending(){if(!state.drag&&!state.draftMark)return false;toast(t('请先松开鼠标完成当前操作，再输出。'));return true;}
function objectUndo(redo=false){if(state.textPoint)return;abortCaptureGesture();const from=redo?state.redo:state.undoSteps,to=redo?state.undoSteps:state.redo;if(from.length){to.push(clone(state.marks));state.marks=from.pop();}state.objectSelected=null;}
function objectBounds(m){
 const size=m.textSize||22,points=m.tool==='pen'?m.points:[m.a,m.b];
 if(m.tool==='text')return {x:m.a.x,y:m.a.y,w:Math.max(size,Array.from(m.text).length*size*.7),h:size*1.3};
 const pad=['rect','pen','arrow'].includes(m.tool)?m.stroke/2+2:0;
 const x=Math.max(0,Math.min(...points.map(p=>p.x))-pad),y=Math.max(0,Math.min(...points.map(p=>p.y))-pad);
 return {x,y,w:Math.max(...points.map(p=>p.x))+pad-x,h:Math.max(...points.map(p=>p.y))+pad-y};
}
function objectHit(m,p){
 const b=objectBounds(m),tol=5;if(p.x<b.x-tol||p.y<b.y-tol||p.x>b.x+b.w+tol||p.y>b.y+b.h+tol)return false;
 const segment=(a,b)=>{const dx=b.x-a.x,dy=b.y-a.y,len=dx*dx+dy*dy,t=len?Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/len)):0;return Math.hypot(p.x-a.x-t*dx,p.y-a.y-t*dy)<=m.stroke/2+tol;};
 if(m.tool==='pen')return m.points.some((a,i)=>segment(a,m.points[i+1]||a));
 if(m.tool==='rect'){const a=m.a,b=m.b,c={x:b.x,y:a.y},d={x:a.x,y:b.y};return segment(a,c)||segment(c,b)||segment(b,d)||segment(d,a);}
 if(m.tool==='arrow'){const a=m.a,b=m.b,angle=Math.atan2(b.y-a.y,b.x-a.x),len=Math.min(18,Math.hypot(b.x-a.x,b.y-a.y)),wing=v=>({x:b.x-len*Math.cos(v),y:b.y-len*Math.sin(v)});return segment(a,b)||segment(wing(angle-.5),b)||segment(wing(angle+.5),b);}
 return true;
}
function moveObject(m,dx,dy,r){
 const b=objectBounds(m),out=clone(m);
 dx=b.w>r.w?0:Math.max(-b.x,Math.min(Math.max(0,r.w-b.w-b.x),dx));dy=b.h>r.h?0:Math.max(-b.y,Math.min(Math.max(0,r.h-b.h-b.y),dy));
 for(const p of [out.a,out.b,...(out.points||[])]){p.x+=dx;p.y+=dy;}return out;
}
function objectFrame(m){if(m.tool==='pen'){const x=Math.min(...m.points.map(p=>p.x)),y=Math.min(...m.points.map(p=>p.y));return {x,y,w:Math.max(...m.points.map(p=>p.x))-x,h:Math.max(...m.points.map(p=>p.y))-y};}const x=Math.min(m.a.x,m.b.x),y=Math.min(m.a.y,m.b.y);return {x,y,w:Math.abs(m.a.x-m.b.x),h:Math.abs(m.a.y-m.b.y)};}
function objectGrips(m){if(m.tool==='text')return [];if(m.tool==='arrow')return [{id:8,...m.a},{id:9,...m.b}];const b=objectFrame(m);return [[0,0],[.5,0],[1,0],[1,.5],[1,1],[.5,1],[0,1],[0,.5]].map(([x,y],id)=>({id,x:b.x+b.w*x,y:b.y+b.h*y})).filter(g=>b.w===0&&b.h===0?false:b.w===0?[1,5].includes(g.id):b.h===0?[3,7].includes(g.id):true);}
function objectGripHit(m,p,r){return objectGrips(m).filter(g=>g.x>=0&&g.y>=0&&g.x<=r.w&&g.y<=r.h&&Math.abs(g.x-p.x)<=6&&Math.abs(g.y-p.y)<=6).sort((a,b)=>Math.hypot(a.x-p.x,a.y-p.y)-Math.hypot(b.x-p.x,b.y-p.y))[0];}
function beginObjectResize(p,r){if(!r||state.tool!=='select'||state.objectSelected===null)return false;const m=state.marks[state.objectSelected],g=objectGripHit(m,{x:p.x-r.x,y:p.y-r.y},r);if(!g)return false;state.drag={mode:'object',start:p,original:clone(r),moved:false,object:clone(m),handle:g.id,before:clone(state.marks)};return true;}
function resizeObject(m,handle,dx,dy,r){
 const out=clone(m);if(!dx&&!dy)return out;
 if(m.tool==='arrow'){const p=handle===8?out.a:out.b;p.x=Math.max(0,Math.min(r.w,p.x+dx));p.y=Math.max(0,Math.min(r.h,p.y+dy));return out.a.x===out.b.x&&out.a.y===out.b.y?null:out;}
 const b=objectFrame(m);let l=b.x,t=b.y,right=l+b.w,bottom=t+b.h;
 if([0,6,7].includes(handle))l=Math.max(0,Math.min(right-1,r.w,l+dx));if([2,3,4].includes(handle))right=Math.max(l+1,Math.min(r.w,right+dx));
 if([0,1,2].includes(handle))t=Math.max(0,Math.min(bottom-1,r.h,t+dy));if([4,5,6].includes(handle))bottom=Math.max(t+1,Math.min(r.h,bottom+dy));
 if(m.tool==='pen'){out.points=out.points.map(p=>({x:b.w?l+(p.x-b.x)*(right-l)/b.w:p.x,y:b.h?t+(p.y-b.y)*(bottom-t)/b.h:p.y}));out.a=clone(out.points[0]);out.b=clone(out.points.at(-1));}
 else{out.a={x:m.a.x<=m.b.x?l:right,y:m.a.y<=m.b.y?t:bottom};out.b={x:m.a.x<=m.b.x?right:l,y:m.a.y<=m.b.y?bottom:t};}return out;
}
function objectOutline(){const m=state.tool==='select'?state.marks[state.objectSelected]:null;if(!m)return '';const b=objectBounds(m),r=state.selection;return `<div class="object-outline" style="left:${b.x}px;top:${b.y}px;width:${b.w}px;height:${b.h}px"></div>${objectGrips(m).filter(g=>g.x>=0&&g.y>=0&&g.x<=r.w&&g.y<=r.h).map(g=>`<i class="object-grip" data-grip="${g.id}" style="left:${g.x}px;top:${g.y}px"></i>`).join('')}`;}
function deleteObject(){if(state.textPoint)return;abortObjectDrag();if(state.objectSelected!==null&&state.marks[state.objectSelected]){checkpoint();state.marks.splice(state.objectSelected,1);state.objectSelected=null;}render();}
function styleObject(style){if(state.textPoint)return;abortObjectDrag();const m=state.marks[state.objectSelected];if(!m||m.tool==='mosaic')return;const changed={...clone(m),...style};if(JSON.stringify(m)!==JSON.stringify(changed)){checkpoint();state.marks[state.objectSelected]=changed;}render();}
function editObjectText(){const m=state.marks[state.objectSelected];if(m?.tool!=='text'||state.textPoint)return;state.textTarget=state.objectSelected;state.textPoint=clone(m.a);state.textDraft=m.text;state.textFont=m.textFont;state.textSize=m.textSize;state.color=m.color;render();}
function textColors(){const colors=['#ed6571','#4b5cfa','#13a88a','#ffb347','#ffffff','#202b42'];if(!colors.includes(saved.markColor.toLowerCase()))colors.unshift(saved.markColor);return colors;}
function objectPanel(){const m=state.marks[state.objectSelected],disabled=state.textPoint?'disabled':'';return `<div class="row object-properties"><span>${m.tool==='text'?'拖动移动文字 · 编辑文字可调整字号 · Delete 删除':'拖动移动标注 · 手柄调整形状 / 箭头端点 · Delete 删除'}</span>${m.tool==='mosaic'?'':`<div class="row">${textColors().map(c=>`<button class="swatch ${m.color===c?'active':''}" style="background:${c}" data-action="object-color" data-value="${c}" aria-label="${c}" ${disabled}></button>`).join('')}</div>`}${['rect','arrow','pen'].includes(m.tool)?`<label>线宽 <select id="object-stroke" ${disabled}>${Array.from({length:12},(_,i)=>i+1).map(n=>`<option ${n===m.stroke?'selected':''}>${n}</option>`).join('')}</select></label>`:''}${m.tool==='arrow'?`<label>箭头样式 <select id="object-arrow" ${disabled}>${['实心箭头','开放箭头'].map(s=>`<option value="${esc(s)}" ${s===m.arrowStyle?'selected':''}>${s}</option>`).join('')}</select></label>`:''}${m.tool==='text'?`<button data-action="edit-object-text" ${disabled}>编辑文字</button>`:''}<button data-action="delete-object" ${disabled}>删除标注</button></div>`;}
function inlineTextStyle(){return `<div class="row"><label>字号 <select id="inline-text-size">${Array.from({length:37},(_,i)=>i+12).map(n=>`<option ${n===state.textSize?'selected':''}>${n}</option>`).join('')}</select></label>${textColors().map(c=>`<button class="swatch ${state.color===c?'active':''}" style="background:${c}" data-action="mark-color" data-value="${c}" aria-label="${c}"></button>`).join('')}</div>`;}
