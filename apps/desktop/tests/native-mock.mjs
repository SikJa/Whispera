export function installNativeMock({kind='video',width=640,height=480,scale=1,setupComplete=true,selectionPhase,frozenImage}={}) {
  window.calls=[]; const callbacks=new Map(), listeners=new Map();let next=1;
  window.videoPreferences={audio:'none',hotkey:'Control+Shift+F9',image_hotkey:'Control+Shift+F10',frame_color:'#ffffff',image_auto_copy:false};
  window.voiceShortcut='Alt+KeyZ';
  window.libraryPreferences={toggleHotkey:'Alt+C',captureGlobal:true,incognito:false,historyLimit:250,autoDeleteHours:48};
  window.videoStatus={phase:selectionPhase??(new URLSearchParams(location.search).get('view')==='screen-select'?'selecting':'idle'),seconds:0,path:'',error:'',copied:false};
  window.editorContext={id:'test-session',kind,width,height,scale};
  window.feedback={};window.exports=[];window.failExport=false;
  window.emitNative=(event,payload)=>{for(const [id,v] of listeners)if(v.event===event)callbacks.get(v.handler)?.({event,id,payload});};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:(_,id)=>listeners.delete(id)};
  window.__TAURI_INTERNALS__={
    metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},
    transformCallback:callback=>{const id=next++;callbacks.set(id,callback);return id;},
    invoke:async(command,args={})=>{
      window.calls.push({command,args});
      if(command==='plugin:event|listen'){const id=next++;listeners.set(id,args);return id;}
      if(command==='plugin:event|unlisten')return;
      if(command==='setup_info')return{complete:setupComplete||localStorage.getItem('test.setup.complete')==='true',startup:false,microphone:'Micrófono de prueba'};
      if(command==='complete_setup')localStorage.setItem('test.setup.complete','true');
      if(command==='snapshot')return{settings:{color:'#9024DC',hotkey:window.voiceShortcut,language:'es',autoPaste:true},rules:[],history:[],logs:[],keyConfigured:false,native:true};
      if(command==='screen_appearance')return{color:'#9024DC',recorderScale:.85,pattern:'wave'};
      if(command==='read_settings')return{color:'#9024DC',recorderScale:.85,pattern:'wave'};
      if(command==='screen_recent')return[{id:'recent-1',kind:'image',created_at:'2026-10-02T00:00:00Z',path:'test.png'}];
      if(command==='screen_selection_kind')return kind;
      if(command==='screen_selection_image')return frozenImage?new Uint8Array(frozenImage).buffer:new ArrayBuffer(0);
      if(command==='screen_preferences')return window.videoPreferences;
      if(command==='library_preferences')return window.libraryPreferences;
      if(command==='library_action'&&args.action==='settings'){window.libraryPreferences={...window.libraryPreferences,...args.value};return window.libraryPreferences;}
      if(command==='screen_status')return window.videoStatus;
      if(command==='screen_audio_devices')return{microphone:'Micrófono de prueba',system:'Altavoces de prueba'};
      if(command==='save_all_shortcuts'){window.voiceShortcut=args.voice;window.videoPreferences={...window.videoPreferences,hotkey:args.video,image_hotkey:args.image};window.libraryPreferences.toggleHotkey=args.library;}
      if(command==='screen_pause')window.videoStatus.phase=window.videoStatus.phase==='paused'?'recording':'paused';
      if(command==='screen_cancel')window.videoStatus.phase='idle';
      if(command==='screen_stop')window.videoStatus.phase='idle';
      if(command==='screen_video_snapshot')return true;
      if(command==='screen_tools_panel')return {railX:4,railY:4,menuX:58,menuY:Math.max(4,Math.min(innerHeight-args.panelHeight+4,args.anchor-args.panelHeight/2))};
      if(command==='screen_save_preferences')window.videoPreferences=args.preferences;
      if(command==='screen_start'){window.videoStatus.phase=kind==='image'?'editing':'recording';window.emitNative('screen-stage',{rect:args.rect,kind});}
      if(command==='screen_editor_context')return window.editorContext;
      if(command==='screen_editor_action')window.emitNative('screen-editor-action',args);
      if(command==='screen_editor_feedback'){window.feedback=args.feedback;window.emitNative('screen-editor-feedback',args);}
      if(command==='screen_editor_feedback_get')return window.feedback;
      if(command==='screen_editor_image'||command==='screen_editor_sample'){
        const canvas=document.createElement('canvas');canvas.width=(args.rect?.width??width)*scale;canvas.height=(args.rect?.height??height)*scale;
        const ctx=canvas.getContext('2d');ctx.fillStyle='#eeeeee';ctx.fillRect(0,0,canvas.width,canvas.height);
        if(command==='screen_editor_sample'){ctx.fillStyle='#3355aa';ctx.fillRect(0,0,canvas.width/2,canvas.height);}
        return Array.from(Uint8Array.from(atob(canvas.toDataURL('image/png').split(',')[1]),v=>v.charCodeAt(0)));
      }
      if(command==='screen_image_export'){if(window.failExport)throw Error('Portapapeles ocupado');window.exports.push(args);return true;}
    }
  };
}
