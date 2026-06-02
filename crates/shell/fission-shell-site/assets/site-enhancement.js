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

  function boot(){
    document.querySelectorAll('.fission-site-doc-sidebar').forEach(initSidebar);
    document.querySelectorAll('.fission-site-doc-nav,.fission-site-main-nav,.fission-site-mobile-global-menu').forEach(initNav);
  }
  if(document.readyState==='loading'){
    document.addEventListener('DOMContentLoaded',boot,{once:true});
  }else{
    boot();
  }
}());
