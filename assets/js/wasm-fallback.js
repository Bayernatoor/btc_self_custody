// Fallback for browsers that don't support WebAssembly (e.g. GrapheneOS
// Vanadium with JIT disabled).  Shows a banner explaining how to restore
// full functionality and wires up the hamburger menu with vanilla JS so
// users can at least navigate the site.
(function () {
  if (typeof WebAssembly === 'object') return;

  // --- Banner -----------------------------------------------------------
  function showBanner() {
    var banner = document.createElement('div');
    banner.id = 'wasm-fallback-banner';
    banner.setAttribute('role', 'alert');
    banner.style.cssText =
      'position:fixed;bottom:0;left:0;right:0;z-index:9999;' +
      'background:#1a1a2e;border-top:1px solid rgba(247,147,26,0.3);' +
      'padding:12px 16px;display:flex;align-items:flex-start;gap:10px;' +
      'font-family:system-ui,sans-serif;font-size:13px;color:rgba(255,255,255,0.85);' +
      'line-height:1.45;';

    banner.innerHTML =
      '<div style="flex:1">' +
        '<strong style="color:#f7931a">Limited functionality</strong><br>' +
        'Your browser has WebAssembly disabled. Interactive features ' +
        '(guides, FAQ, step navigation) require it.<br>' +
        '<span style="color:rgba(255,255,255,0.55);font-size:12px">' +
          'Vanadium: tap \u22ee \u2192 Settings \u2192 Site settings \u2192 JavaScript JIT \u2192 enable for this site, then reload.' +
        '</span>' +
      '</div>' +
      '<button id="wasm-banner-close" style="' +
        'background:none;border:1px solid rgba(255,255,255,0.15);' +
        'color:rgba(255,255,255,0.5);border-radius:6px;padding:4px 10px;' +
        'font-size:12px;cursor:pointer;white-space:nowrap;flex-shrink:0;' +
      '">Dismiss</button>';

    document.body.appendChild(banner);

    document.getElementById('wasm-banner-close').addEventListener('click', function () {
      banner.remove();
    });
  }

  // --- Hamburger menu ---------------------------------------------------
  function fixHamburger() {
    var btn = document.getElementById('navbar_hamburger_menu');
    if (!btn) return;

    // The mobile dropdown is the next sibling div after the navbar's
    // inner flex container.  Leptos SSR renders it with the "hidden" class.
    var nav = btn.closest('nav');
    if (!nav) return;

    var dropdown = nav.querySelector('.fixed.min-w-44');
    if (!dropdown) return;

    btn.addEventListener('click', function () {
      dropdown.classList.toggle('hidden');
    });

    // Close when a link inside the dropdown is clicked
    dropdown.querySelectorAll('a').forEach(function (a) {
      a.addEventListener('click', function () {
        dropdown.classList.add('hidden');
      });
    });

    // Close on outside click
    document.addEventListener('click', function (e) {
      if (!nav.contains(e.target)) {
        dropdown.classList.add('hidden');
      }
    });
  }

  // --- Init -------------------------------------------------------------
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () {
      showBanner();
      fixHamburger();
    });
  } else {
    showBanner();
    fixHamburger();
  }
})();
