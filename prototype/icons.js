/* SPDX-License-Identifier: GPL-3.0-only. Shared Glint action icon geometry. */
(() => {
  'use strict';
  const paths = {
    "select": "M4 3.5 10 20l2.5-7.5L20 10Z",
    "rect": "M6 4h12a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2Z",
    "arrow": "M5 19 19 5M9 5h10v10",
    "pen": "m4 20 1.5-5.5L16 4a2.1 2.1 0 0 1 3 3L8.5 17.5 4 20Zm10-14 4 4",
    "text": "M4 7V4h16v3M12 4v16M8 20h8",
    "highlight": "m6 15 8-11 5 4-8 11-5-4Zm-2 5 2-5M3 21h15",
    "redact": "M5 5h14a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1ZM8 9h8M8 12h8M8 15h8",
    "mosaic": "M4 4h6v6H4Zm10 0h6v6h-6ZM4 14h6v6H4Zm10 0h6v6h-6Z",
    "picker": "m4 20 2-5L16 5l3 3L9 18l-5 2ZM14 4l6 6",
    "undo": "m9 5-5 5 5 5M4 10h9a5 5 0 0 1 0 10h-3",
    "redo": "m15 5 5 5-5 5M20 10h-9a5 5 0 0 0 0 10h3",
    "save": "M5 3h12l3 3v14H4V4a1 1 0 0 1 1-1ZM8 3v6h8V3M8 20v-6h8v6",
    "copy": "M9 8h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1ZM15 8V4H4v11h4",
    "close": "m6 6 12 12M18 6 6 18",
    "more": "M4 12h.5m7.25 0h.5m7.25 0h.5",
    "settings": "M4 6h16M4 12h16M4 18h16M8 3v6M16 9v6M10 15v6",
    "pin": "M8 4h8l-1 6 4 4H5l4-4-1-6ZM12 14v7",
    "check": "m5 12 4 4L19 6",
    "compare": "M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1ZM12 4v16M7 9l2-2m6 9 2-2",
    "fit": "M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5",
    "crop": "M7 3v14h14M3 7h14v14",
    "shape": "M4 4h10v10H4Zm12 8a4 4 0 1 1-4 4",
    "adjust": "M4 6h16M4 12h16M4 18h16M8 3v6M16 9v6M10 15v6",
    "clip": "M5 5h14a1 1 0 0 1 1 1v13H4V6a1 1 0 0 1 1-1ZM4 10h16M8 5l3 5m3-5 3 5",
    "audio": "M4 9h4l5-4v14l-5-4H4V9Zm12-1a6 6 0 0 1 0 8m2-11a10 10 0 0 1 0 14",
    "muted": "M4 9h4l5-4v14l-5-4H4V9Zm12 0 5 6m0-6-5 6",
    "rotate": "M19 9a7 7 0 1 0 .1 6M19 4v5h-5",
    "flip": "M12 3v18M3 17l5-10v10H3Zm18 0L16 7v10h5Z",
    "trash": "M3 6h18M9 6V3h6v3M5 6l1 14h12l1-14M10 10v6m4-6v6",
    "play": "m8 5 11 7-11 7V5Z",
    "pause": "M8 5v14M16 5v14",
    "frame_back": "M5 5v14M19 5l-10 7 10 7V5Z",
    "frame_next": "M19 5v14M5 5l10 7-10 7V5Z",
    "split": "M9 8 20 19M9 16 20 5M4 4a3 3 0 1 0 0 6 3 3 0 0 0 0-6Zm0 10a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z",
    "plus": "M5 12h14M12 5v14",
    "minus": "M5 12h14",
    "image": "M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1ZM4 16l5-5 4 4 3-3 4 4M15 8h.5",
    "video": "M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1ZM8 4v16M16 4v16M4 8h4m8 0h4M4 16h4m8 0h4",
    "folder": "M3 7V5h7l2 3h9l-3 12H3V7Zm0 3h7",
    "layers": "m12 3 10 5-10 5L2 8l10-5ZM2 12l10 5 10-5M2 16l10 5 10-5",
    "resize": "M9 4H4v5M4 4l6 6m5 10h5v-5m0 5-6-6",
    "grip": "M8 5h.5m7 0h.5M8 12h.5m7 0h.5M8 19h.5m7 0h.5",
    "up": "m6 14 6-6 6 6",
    "down": "m6 10 6 6 6-6"
  };
  const aliases={'finish':'check','mouse-pointer-2':'select','square':'rect','move-up-right':'arrow','pencil':'pen','highlighter':'highlight','type':'text','scan-line':'redact','grid-2x2':'mosaic','sliders-horizontal':'adjust','clapperboard':'clip','volume-2':'audio','volume-x':'muted','x':'close','rotate-cw':'rotate','flip-horizontal-2':'flip','trash-2':'trash','film':'video','undo-2':'undo','redo-2':'redo','columns-2':'compare','ellipsis':'more','maximize':'fit','scissors':'split','skip-back':'frame_back','skip-forward':'frame_next','folder-open':'folder','scaling':'resize'};
  const svg=(name,size=18)=>`<svg class="glint-icon" viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${paths[aliases[name]||name]||paths.more}"/></svg>`;
  const render=(root=document)=>root.querySelectorAll('[data-glint-icon]').forEach(node=>{node.outerHTML=svg(node.dataset.glintIcon);});
  globalThis.GlintIcons=Object.freeze({paths,svg,render});
})();
