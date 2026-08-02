(function(){
  var d=document.documentElement;
  d.classList.add('fission-site-js');

  function initSidebar(root){
    var items=Array.prototype.slice.call(root.querySelectorAll('.fission-site-sidebar-item'));
    if(!items.length)return;
    var sk='fission-site-sidebar:v2:'+location.pathname;
    var expanded=new Set();
    try{JSON.parse(localStorage.getItem(sk)||'[]').forEach(function(v){expanded.add(String(v));});}catch(_){}
    function level(el){return Number(el.dataset.fissionSiteSidebarLevel||'0');}
    function group(el){return el.dataset.fissionSiteSidebarGroup==='true';}
    function active(el){return el.dataset.fissionSiteSidebarActive==='true';}
    function hasChildren(i){
      var l=level(items[i]);
      for(var j=i+1;j<items.length;j++){
        if(level(items[j])<=l)return false;
        if(level(items[j])===l+1)return true;
      }
      return false;
    }
    function ancestors(i){
      var out=[],current=level(items[i]);
      for(var j=i-1;j>=0;j--){
        var l=level(items[j]);
        if(l<current){out.unshift(j);current=l;if(current===0)break;}
      }
      return out;
    }
    items.forEach(function(el,i){
      el.dataset.fissionSiteSidebarIndex=String(i);
      if(active(el)){
        ancestors(i).forEach(function(a){expanded.add(String(a));});
        if(group(el))expanded.add(String(i));
      }
    });
    function apply(){
      items.forEach(function(el,i){
        var visible=level(el)===0||ancestors(i).every(function(a){return expanded.has(String(a));});
        el.hidden=!visible;
        el.dataset.fissionSiteSidebarExpanded=expanded.has(String(i))?'true':'false';
        el.dataset.fissionSiteSidebarHasChildren=hasChildren(i)?'true':'false';
      });
      try{localStorage.setItem(sk,JSON.stringify(Array.from(expanded)));}catch(_){}
    }
    root.addEventListener('click',function(e){
      var item=e.target.closest('.fission-site-sidebar-item');
      if(!item||!root.contains(item))return;
      var i=items.indexOf(item);
      if(i<0||!hasChildren(i))return;
      if(!expanded.has(String(i))){e.preventDefault();expanded.add(String(i));apply();}
    });
    apply();
  }

  function setDrawer(open){
    d.classList.toggle('fission-site-sidebar-open',open);
    document.querySelectorAll('[data-fission-sidebar-toggle]').forEach(function(button){
      button.setAttribute('aria-expanded',open?'true':'false');
    });
  }

  function initNav(root){
    root.addEventListener('click',function(e){
      var item=e.target.closest('.fission-site-nav-item[data-fission-site-nav-has-children="true"]');
      if(!item||!root.contains(item))return;
      var nestedMenu=e.target.closest('.fission-site-nav-menu');
      var nestedItem=e.target.closest('.fission-site-nav-item');
      if(nestedMenu&&nestedItem!==item)return;
      var alreadyOpen=item.dataset.fissionSiteNavOpen==='true';
      if(!alreadyOpen){
        e.preventDefault();
        root.querySelectorAll('.fission-site-nav-item[data-fission-site-nav-open="true"]').forEach(function(openItem){
          if(openItem!==item&&!openItem.contains(item))openItem.removeAttribute('data-fission-site-nav-open');
        });
        item.dataset.fissionSiteNavOpen='true';
      }
    });
  }

  document.addEventListener('click',function(e){
    var toggle=e.target.closest('[data-fission-sidebar-toggle]');
    if(toggle){
      e.preventDefault();
      setDrawer(!d.classList.contains('fission-site-sidebar-open'));
      return;
    }
    if(!e.target.closest('.fission-site-nav-item')){
      document.querySelectorAll('.fission-site-nav-item[data-fission-site-nav-open="true"]').forEach(function(item){
        item.removeAttribute('data-fission-site-nav-open');
      });
    }
    if(d.classList.contains('fission-site-sidebar-open')){
      var sidebar=e.target.closest('.fission-site-doc-sidebar');
      if(sidebar)return;
      setDrawer(false);
    }
  });

  document.addEventListener('keydown',function(e){
    if(e.key==='Escape'){
      setDrawer(false);
      document.querySelectorAll('.fission-site-nav-item[data-fission-site-nav-open="true"]').forEach(function(item){
        item.removeAttribute('data-fission-site-nav-open');
      });
    }
  });

  function markerData(node,language){
    if(!node||node.nodeType!==1||!node.querySelector)return null;
    var code=node.querySelector('code.language-'+language);
    if(!code)return null;
    try{return JSON.parse(code.textContent.trim());}catch(_){return null;}
  }

  function removeNode(node){
    if(node&&node.parentNode)node.parentNode.removeChild(node);
  }

  function initTabs(root){
    var starts=Array.prototype.slice.call(root.querySelectorAll('code.language-fission-tabs-start'));
    starts.forEach(function(code,startIndex){
      var startBlock=code.closest('.fission-site-code-block')||code.closest('pre');
      if(!startBlock||startBlock.dataset.fissionTabsHydrated==='true')return;
      startBlock.dataset.fissionTabsHydrated='true';
      var meta=markerData(startBlock,'fission-tabs-start');
      if(!meta||!Array.isArray(meta.tabs))return;
      var parent=startBlock.parentNode;
      if(!parent)return;

      var markers=[startBlock];
      var panels=[];
      var current=null;
      var node=startBlock.nextSibling;
      while(node){
        var next=node.nextSibling;
        var tabMeta=markerData(node,'fission-tab-start');
        if(tabMeta){
          markers.push(node);
          current={meta:tabMeta,nodes:[]};
          panels.push(current);
          node=next;
          continue;
        }
        if(markerData(node,'fission-tabs-end')){
          markers.push(node);
          node=next;
          break;
        }
        if(current){
          current.nodes.push(node);
        }else if(node.nodeType===3&&!node.textContent.trim()){
          markers.push(node);
        }
        node=next;
      }
      if(!panels.length)return;

      var container=document.createElement('div');
      var base='fission-doc-tabs-'+(meta.id||String(startIndex)).replace(/[^A-Za-z0-9_-]/g,'-');
      container.className='fission-doc-tabs';
      container.dataset.fissionTabs='true';
      var tabList=document.createElement('div');
      tabList.className='fission-doc-tab-list';
      tabList.setAttribute('role','tablist');
      container.appendChild(tabList);

      var buttons=[];
      var panelEls=[];
      function select(index,focus){
        buttons.forEach(function(button,i){
          var selected=i===index;
          button.setAttribute('aria-selected',selected?'true':'false');
          button.tabIndex=selected?0:-1;
          panelEls[i].hidden=!selected;
        });
        if(focus)buttons[index].focus();
      }

      panels.forEach(function(panel,index){
        var label=panel.meta.label||(meta.tabs[index]&&meta.tabs[index].label)||('Tab '+(index+1));
        var button=document.createElement('button');
        var tabId=base+'-tab-'+index;
        var panelId=base+'-panel-'+index;
        button.type='button';
        button.className='fission-doc-tab';
        button.id=tabId;
        button.textContent=label;
        button.setAttribute('role','tab');
        button.setAttribute('aria-controls',panelId);
        button.addEventListener('click',function(){select(index,false);});
        button.addEventListener('keydown',function(e){
          var nextIndex=index;
          if(e.key==='ArrowRight')nextIndex=(index+1)%buttons.length;
          else if(e.key==='ArrowLeft')nextIndex=(index+buttons.length-1)%buttons.length;
          else if(e.key==='Home')nextIndex=0;
          else if(e.key==='End')nextIndex=buttons.length-1;
          else return;
          e.preventDefault();
          select(nextIndex,true);
        });
        buttons.push(button);
        tabList.appendChild(button);

        var panelEl=document.createElement('div');
        panelEl.className='fission-doc-tab-panel';
        panelEl.id=panelId;
        panelEl.setAttribute('role','tabpanel');
        panelEl.setAttribute('aria-labelledby',tabId);
        panel.nodes.forEach(function(child){panelEl.appendChild(child);});
        panelEls.push(panelEl);
        container.appendChild(panelEl);
      });

      parent.insertBefore(container,startBlock);
      markers.forEach(removeNode);
      select(0,false);
    });
  }

  function findFissionNode(root,id){
    var nodes=root.querySelectorAll('[data-fission-node]');
    for(var i=0;i<nodes.length;i++){
      if(nodes[i].getAttribute('data-fission-node')===id)return nodes[i];
    }
    return null;
  }

  function initSpotlights(root){
    var spotlights=Array.prototype.slice.call(root.querySelectorAll('[data-fission-spotlight-anchor]'));
    if(!spotlights.length)return;
    var frame=0;

    function place(region,left,top,width,height){
      region.style.position='absolute';
      region.style.left=Math.max(0,left)+'px';
      region.style.top=Math.max(0,top)+'px';
      region.style.width=Math.max(0,width)+'px';
      region.style.height=Math.max(0,height)+'px';
      region.hidden=width<=0||height<=0;
    }

    function layout(spotlight){
      var regions=Array.prototype.slice.call(spotlight.children);
      if(regions.length!==5)return;
      var bounds=spotlight.getBoundingClientRect();
      var anchor=findFissionNode(document,spotlight.getAttribute('data-fission-spotlight-anchor'));
      if(!anchor){
        place(regions[0],0,0,bounds.width,bounds.height);
        for(var missing=1;missing<regions.length;missing++)place(regions[missing],0,0,0,0);
        spotlight.dataset.fissionSpotlightState='anchor-missing';
        return;
      }

      var target=anchor.getBoundingClientRect();
      var padding=Number(spotlight.getAttribute('data-fission-spotlight-padding')||'0');
      if(!Number.isFinite(padding))padding=0;
      padding=Math.max(0,padding);
      var left=Math.max(0,Math.min(bounds.width,target.left-bounds.left-padding));
      var top=Math.max(0,Math.min(bounds.height,target.top-bounds.top-padding));
      var right=Math.max(0,Math.min(bounds.width,target.right-bounds.left+padding));
      var bottom=Math.max(0,Math.min(bounds.height,target.bottom-bounds.top+padding));
      if(right<=left||bottom<=top){
        place(regions[0],0,0,bounds.width,bounds.height);
        for(var invalid=1;invalid<regions.length;invalid++)place(regions[invalid],0,0,0,0);
        spotlight.dataset.fissionSpotlightState='anchor-hidden';
        return;
      }

      place(regions[0],0,0,bounds.width,top);
      place(regions[1],0,bottom,bounds.width,bounds.height-bottom);
      place(regions[2],0,top,left,bottom-top);
      place(regions[3],right,top,bounds.width-right,bottom-top);
      place(regions[4],left,top,right-left,bottom-top);
      spotlight.dataset.fissionSpotlightState='ready';
    }

    function update(){
      frame=0;
      spotlights.forEach(layout);
    }
    function schedule(){
      if(frame)return;
      frame=requestAnimationFrame(update);
    }

    update();
    window.addEventListener('resize',schedule,{passive:true});
    window.addEventListener('scroll',schedule,{passive:true,capture:true});
    if(typeof ResizeObserver!=='undefined'){
      var observer=new ResizeObserver(schedule);
      spotlights.forEach(function(spotlight){
        observer.observe(spotlight);
        var anchor=findFissionNode(document,spotlight.getAttribute('data-fission-spotlight-anchor'));
        if(anchor)observer.observe(anchor);
      });
    }
  }

  function boot(){
    document.querySelectorAll('[data-fission-locale-switcher]').forEach(function(select){
      var spanish=location.pathname==='/es'||location.pathname.indexOf('/es/')===0;
      select.value=spanish?'es':'en';
      select.addEventListener('change',function(){
        if(select.value==='es'){
          location.href=location.pathname.indexOf('/crates/')===0?'/es/crates/':'/es/';
        }else{
          location.href=location.pathname.indexOf('/es/crates/')===0?'/crates/':'/';
        }
      });
    });
    initTabs(document);
    initSpotlights(document);
    document.querySelectorAll('.fission-site-doc-sidebar').forEach(initSidebar);
    document.querySelectorAll('.fission-site-doc-nav,.fission-site-main-nav,.fission-site-mobile-global-menu').forEach(initNav);
  }
  if(document.readyState==='loading'){
    document.addEventListener('DOMContentLoaded',boot,{once:true});
  }else{
    boot();
  }
}());
