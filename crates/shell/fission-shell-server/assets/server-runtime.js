(function(){
  var textEncoder=new TextEncoder();
  var textDecoder=new TextDecoder();
  var bridgeInstances=[];

  function safeSemantics(value){
    return typeof value==='string'&&value.length>0&&value.length<=160&&/^[A-Za-z0-9_:\/.\-]+$/.test(value);
  }

  function safeAttrName(name){
    return typeof name==='string'&&/^[A-Za-z0-9_:\.\-]+$/.test(name)&&!/^on/i.test(name);
  }

  function safeUrl(value){
    if(typeof value!=='string')return false;
    var lower=value.trimStart().toLowerCase();
    if(lower.indexOf('javascript:')===0||lower.indexOf('data:')===0||lower.indexOf('vbscript:')===0||lower.indexOf('\\')!==-1)return false;
    return lower.indexOf('/')===0&&lower.indexOf('//')!==0||lower.indexOf('https://')===0||lower.indexOf('http://')===0;
  }

  function cssEscape(value){
    if(window.CSS&&typeof window.CSS.escape==='function')return window.CSS.escape(value);
    return String(value).replace(/[^A-Za-z0-9_\-]/g,function(ch){return'\\'+ch;});
  }

  function bySemantics(semantics){
    if(!safeSemantics(semantics))return null;
    return document.querySelector('[data-fission-semantics="'+cssEscape(semantics)+'"]');
  }

  function textTarget(el){
    if(!el)return null;
    var run=el.querySelector('.fission-site-text-run');
    if(run)return run;
    var richText=el.querySelector('.fission-site-rich-text');
    if(richText)return richText;
    return el;
  }

  function setElementText(el,text){
    var target=textTarget(el);
    if(target)target.textContent=String(text==null?'':text);
  }

  function setTextBySemantics(semantics,text){
    var el=bySemantics(semantics);
    setElementText(el,text);
  }

  function getMemory(exports){
    if(exports&&exports.memory instanceof WebAssembly.Memory)return exports.memory;
    return null;
  }

  function readOutput(exports){
    var ptrFn=exports.fission_bridge_output_ptr;
    var lenFn=exports.fission_bridge_output_len;
    var memory=getMemory(exports);
    if(typeof ptrFn!=='function'||typeof lenFn!=='function'||!memory)return null;
    var ptr=ptrFn();
    var len=lenFn();
    if(!len)return null;
    var bytes=new Uint8Array(memory.buffer,ptr,len);
    var json=textDecoder.decode(bytes);
    return JSON.parse(json);
  }

  function callBridge(bridge,exportName,payload){
    var exports=bridge.exports;
    var fn=exports[exportName];
    var alloc=exports.fission_bridge_alloc;
    var dealloc=exports.fission_bridge_dealloc;
    var memory=getMemory(exports);
    if(typeof fn!=='function'||typeof alloc!=='function'||typeof dealloc!=='function'||!memory){
      throw new Error('artifact does not export the Fission browser bridge ABI');
    }
    var data=textEncoder.encode(JSON.stringify(payload||{}));
    var ptr=alloc(data.length);
    new Uint8Array(memory.buffer,ptr,data.length).set(data);
    try{
      var code=fn(ptr,data.length);
      var output=readOutput(exports);
      if(output)applyBridgeOutput(bridge,output);
      if(code!==0)throw new Error('artifact returned bridge error '+code);
      return output;
    }finally{
      dealloc(ptr,data.length);
    }
  }

  function dispatchBridgeEvent(bridge,payload){
    if(bridge.worker){
      bridge.worker.postMessage({kind:'event',payload:payload});
      return null;
    }
    return callBridge(bridge,bridge.eventExport,payload);
  }

  function normalizeMessages(output){
    if(!output)return[];
    if(Array.isArray(output))return output;
    if(Array.isArray(output.messages))return output.messages;
    if(typeof output.type==='string')return[output];
    return[];
  }

  function normalizeBindings(output){
    if(output&&Array.isArray(output.bindings))return output.bindings;
    return[];
  }

  function applyBridgeOutput(bridge,output){
    normalizeMessages(output).forEach(function(message){applyMessage(bridge,message);});
    normalizeBindings(output).forEach(function(binding){bindEvent(bridge,binding);});
    bindFissionBrowserActions(bridge);
  }

  function applyMessage(bridge,message){
    if(!message||typeof message.type!=='string')return;
    if(message.type==='ready')return;
    if(message.type==='log'){
      var level=message.level||'info';
      var text=message.message||'';
      (console[level]||console.log).call(console,'[fission '+bridge.kind+' '+bridge.id+'] '+text);
      return;
    }
    if(message.type==='error'){
      console.error('[fission '+bridge.kind+' '+bridge.id+'] '+(message.message||'browser artifact error'));
      return;
    }
    if(message.type==='navigate'){
      if(message.url&&safeUrl(message.url)){
        if(message.mode==='replace')window.history.replaceState({},'',message.url);
        else if(message.mode==='full_document')window.location.href=message.url;
        else window.history.pushState({},'',message.url);
      }
      return;
    }
    if(message.type==='dom_batch'&&Array.isArray(message.ops)){
      message.ops.forEach(function(op){applyDomOp(bridge,op);});
    }
  }

  function targetForOp(op){
    if(op&&typeof op.semantics==='string')return bySemantics(op.semantics);
    return null;
  }

  function applyDomOp(bridge,op){
    if(!op||typeof op.op!=='string')return;
    var el=targetForOp(op);
    switch(op.op){
      case'set_text_by_semantics': setElementText(el,op.text); break;
      case'set_attr_by_semantics': if(el&&safeAttrName(op.name)&&(!/^(href|src|xlink:href)$/i.test(op.name)||safeUrl(op.value)))el.setAttribute(op.name,String(op.value==null?'':op.value)); break;
      case'remove_attr_by_semantics': if(el&&safeAttrName(op.name))el.removeAttribute(op.name); break;
      case'add_class_by_semantics': if(el&&typeof op.class==='string')el.classList.add(op.class); break;
      case'remove_class_by_semantics': if(el&&typeof op.class==='string')el.classList.remove(op.class); break;
      case'toggle_class_by_semantics': if(el&&typeof op.class==='string')el.classList.toggle(op.class,!!op.enabled); break;
      case'set_style_var_by_semantics': if(el&&typeof op.name==='string'&&op.name.indexOf('--')===0)el.style.setProperty(op.name,String(op.value==null?'':op.value)); break;
      case'set_hidden_by_semantics': if(el)el.hidden=!!op.hidden; break;
      case'set_value_by_semantics': if(el&&'value'in el)el.value=String(op.value==null?'':op.value); break;
      case'set_checked_by_semantics': if(el&&'checked'in el)el.checked=!!op.checked; break;
      case'focus_by_semantics': if(el&&typeof el.focus==='function')el.focus(); break;
      case'blur_by_semantics': if(el&&typeof el.blur==='function')el.blur(); break;
      case'replace_children_html_by_semantics': if(el&&typeof op.html==='string'){el.innerHTML=op.html;bindFissionBrowserActions(bridge);} break;
      case'set_stylesheet': setStylesheet(op.id,op.css); break;
      case'push_history': if(safeUrl(op.url))window.history.pushState({},'',op.url); break;
      case'replace_history': if(safeUrl(op.url))window.history.replaceState({},'',op.url); break;
      case'announce': announce(op.text,op.politeness); break;
    }
  }

  function setStylesheet(id,css){
    if(typeof id!=='string'||!/^[A-Za-z0-9_\-:.]+$/.test(id)||typeof css!=='string')return;
    var style=document.getElementById(id);
    if(!style){
      style=document.createElement('style');
      style.id=id;
      document.head.appendChild(style);
    }
    style.textContent=css;
  }

  function bindEvent(bridge,binding){
    if(!binding||!safeSemantics(binding.semantics))return;
    var event=String(binding.event||'');
    if(['click','input','change','submit'].indexOf(event)===-1)return;
    var el=bySemantics(binding.semantics);
    if(!el)return;
    var key=binding.semantics+'\u0000'+event+'\u0000'+JSON.stringify(binding.message||{});
    if(bridge.boundEvents[key])return;
    bridge.boundEvents[key]=true;
    var handler=function(domEvent){
      if(event==='submit')domEvent.preventDefault();
      var payload={
        type:'event',
        artifact:{kind:bridge.kind,id:bridge.id},
        binding:{semantics:binding.semantics,event:event,message:binding.message||{}},
        value:domEvent&&domEvent.target&&'value'in domEvent.target?String(domEvent.target.value):null,
        checked:domEvent&&domEvent.target&&'checked'in domEvent.target?!!domEvent.target.checked:null,
        sequence:++bridge.sequence
      };
      try{dispatchBridgeEvent(bridge,payload);}catch(error){setTextBySemantics(bridge.statusSemantics,bridge.kind+' failed: '+error.message);}
    };
    el.addEventListener(event,handler);
    if(event==='click'){
      el.addEventListener('keydown',function(domEvent){
        if(domEvent.key==='Enter'||domEvent.key===' '){
          domEvent.preventDefault();
          handler(domEvent);
        }
      });
    }
  }

  function bindFissionBrowserActions(bridge){
    document.querySelectorAll('[data-fission-browser-action="true"]').forEach(function(el){
      if(el.__fissionBrowserActionBound)return;
      el.__fissionBrowserActionBound=true;
      var handler=function(domEvent){
        domEvent.preventDefault();
        var payload={
          type:'event',
          artifact:{kind:bridge.kind,id:bridge.id},
          binding:{
            semantics:el.getAttribute('data-fission-semantics')||null,
            event:'click',
            message:{
              fission_browser_action:true,
              action_id:el.getAttribute('data-fission-action-id')||'',
              target_node:el.getAttribute('data-fission-action-target')||'',
              payload_hex:el.getAttribute('data-fission-action-payload')||''
            }
          },
          value:null,
          checked:null,
          sequence:++bridge.sequence
        };
        try{dispatchBridgeEvent(bridge,payload);}catch(error){setTextBySemantics(bridge.statusSemantics,bridge.kind+' failed: '+error.message);}
      };
      el.addEventListener('click',handler);
      el.addEventListener('keydown',function(domEvent){
        if(domEvent.key==='Enter'||domEvent.key===' '){
          handler(domEvent);
        }
      });
    });
  }

  function announce(text,politeness){
    var region=document.getElementById('fission-live-region');
    if(!region){
      region=document.createElement('div');
      region.id='fission-live-region';
      region.style.position='absolute';
      region.style.width='1px';
      region.style.height='1px';
      region.style.overflow='hidden';
      region.style.clip='rect(0 0 0 0)';
      document.body.appendChild(region);
    }
    region.setAttribute('aria-live',politeness==='assertive'?'assertive':'polite');
    region.textContent=String(text||'');
  }

  function bridgeBootPayload(config,kind){
    return {
      type:'boot',
      protocol_version:1,
      artifact:{kind:kind,id:config.id},
      route:currentManifest.route||window.location.pathname,
      mount_id:config.mount_id||null,
      root_node_id:config.root_node_id||config.mount_id||null,
      theme_id:document.documentElement.getAttribute('data-theme')||'default',
      locale:document.documentElement.lang||'en',
      props:config.props||{}
    };
  }

  function workerRuntimeSource(){
    return `
var textEncoder=new TextEncoder();
var textDecoder=new TextDecoder();
var bridge=null;
function getMemory(exports){
  if(exports&&exports.memory instanceof WebAssembly.Memory)return exports.memory;
  return null;
}
function readOutput(exports){
  var ptrFn=exports.fission_bridge_output_ptr;
  var lenFn=exports.fission_bridge_output_len;
  var memory=getMemory(exports);
  if(typeof ptrFn!=='function'||typeof lenFn!=='function'||!memory)return null;
  var ptr=ptrFn();
  var len=lenFn();
  if(!len)return null;
  return JSON.parse(textDecoder.decode(new Uint8Array(memory.buffer,ptr,len)));
}
function callBridge(exportName,payload){
  var exports=bridge.exports;
  var fn=exports[exportName];
  var alloc=exports.fission_bridge_alloc;
  var dealloc=exports.fission_bridge_dealloc;
  var memory=getMemory(exports);
  if(typeof fn!=='function'||typeof alloc!=='function'||typeof dealloc!=='function'||!memory)throw new Error('artifact does not export the Fission browser bridge ABI');
  var data=textEncoder.encode(JSON.stringify(payload||{}));
  var ptr=alloc(data.length);
  new Uint8Array(memory.buffer,ptr,data.length).set(data);
  try{
    var code=fn(ptr,data.length);
    var output=readOutput(exports);
    if(output)self.postMessage({kind:'output',output:output});
    if(code!==0)throw new Error('artifact returned bridge error '+code);
  }finally{
    dealloc(ptr,data.length);
  }
}
self.onmessage=function(event){
  var message=event.data||{};
  if(message.kind==='boot'){
    fetch(message.artifact).then(function(response){
      if(!response.ok)throw new Error(message.artifact+' returned '+response.status);
      return response.arrayBuffer();
    }).then(function(bytes){
      return WebAssembly.instantiate(bytes,{});
    }).then(function(result){
      bridge={exports:result.instance.exports,entryExport:message.entryExport,eventExport:message.eventExport};
      callBridge(bridge.entryExport,message.payload);
      self.postMessage({kind:'ready'});
    }).catch(function(error){
      self.postMessage({kind:'error',message:error.message});
    });
  }else if(message.kind==='event'&&bridge){
    try{callBridge(bridge.eventExport,message.payload);}catch(error){self.postMessage({kind:'error',message:error.message});}
  }
};`;
  }

  function instantiateWorkerArtifact(config,statusSemantics){
    var bridge={
      id:config.id,
      kind:'worker',
      config:config,
      worker:null,
      eventExport:'fission_worker_event',
      statusSemantics:statusSemantics,
      sequence:0,
      boundEvents:Object.create(null)
    };
    bridgeInstances.push(bridge);
    var scriptUrl=URL.createObjectURL(new Blob([workerRuntimeSource()],{type:'application/javascript'}));
    var worker=new Worker(scriptUrl);
    bridge.worker=worker;
    worker.onmessage=function(event){
      var message=event.data||{};
      if(message.kind==='output')applyBridgeOutput(bridge,message.output);
      else if(message.kind==='error')setTextBySemantics(statusSemantics,'Worker failed: '+message.message);
      else if(message.kind==='ready')URL.revokeObjectURL(scriptUrl);
    };
    worker.onerror=function(error){
      setTextBySemantics(statusSemantics,'Worker failed: '+(error.message||'browser worker error'));
      URL.revokeObjectURL(scriptUrl);
    };
    worker.postMessage({
      kind:'boot',
      artifact:new URL(config.artifact,window.location.href).href,
      entryExport:'fission_worker_entry',
      eventExport:'fission_worker_event',
      payload:bridgeBootPayload(config,'worker')
    });
    return Promise.resolve(bridge);
  }

  function instantiateArtifact(config,kind){
    if(!config||!config.artifact)return Promise.resolve(null);
    var statusSemantics=(kind==='worker'?'worker-status:':'island-status:')+config.id;
    if(kind==='worker'&&typeof Worker==='function'){
      return instantiateWorkerArtifact(config,statusSemantics);
    }
    return fetch(config.artifact).then(function(response){
      if(!response.ok)throw new Error(config.artifact+' returned '+response.status);
      return response.arrayBuffer();
    }).then(function(bytes){
      return WebAssembly.instantiate(bytes,{});
    }).then(function(result){
      var bridge={
        id:config.id,
        kind:kind,
        config:config,
        exports:result.instance.exports,
        entryExport:kind==='worker'?'fission_worker_entry':'fission_island_entry',
        eventExport:kind==='worker'?'fission_worker_event':'fission_island_event',
        statusSemantics:statusSemantics,
        sequence:0,
        boundEvents:Object.create(null)
      };
      bridgeInstances.push(bridge);
      callBridge(bridge,bridge.entryExport,bridgeBootPayload(config,kind));
      if(kind==='island'){
        setTextBySemantics(statusSemantics,'Island bridge ready');
        var mount=document.getElementById(config.mount_id)||bySemantics(config.mount_id)||bySemantics(statusSemantics);
        if(mount)mount.setAttribute('data-fission-island-loaded','true');
      }
      return bridge;
    });
  }

  var currentManifest={};
  document.addEventListener('DOMContentLoaded',function(){
    var manifestEl=document.getElementById('fission-route-manifest');
    if(!manifestEl)return;
    try{currentManifest=JSON.parse(manifestEl.textContent||'{}');}catch(error){return;}
    (currentManifest.workers||[]).forEach(function(worker){
      instantiateArtifact(worker,'worker').catch(function(error){setTextBySemantics('worker-status:'+worker.id,'Worker failed: '+error.message);});
    });
    (currentManifest.islands||[]).forEach(function(island){
      instantiateArtifact(island,'island').catch(function(error){setTextBySemantics('island-status:'+island.id,'Island failed: '+error.message);});
    });
  });
}());
