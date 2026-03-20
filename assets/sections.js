// Collapsible sections within step content.
// Turns <h5> tags inside .step-content into clickable section headers
// that expand/collapse the content between them.
(function(){
  // Immediately hide containers that have h5 tags (prevents flash)
  function hideUnprocessed() {
    document.querySelectorAll('.step-content:not([data-sections-init])').forEach(function(c) {
      if (c.querySelectorAll('h5').length > 0) {
        c.classList.add('has-sections');
      }
    });
  }

  function initSections(container) {
    if (container.dataset.sectionsInit) return;

    var headings = container.querySelectorAll('h5');
    if (headings.length === 0) return;

    container.dataset.sectionsInit = 'true';

    headings.forEach(function(h5, index) {
      var content = [];
      var sibling = h5.nextElementSibling;
      while (sibling && sibling.tagName !== 'H5') {
        content.push(sibling);
        sibling = sibling.nextElementSibling;
      }
      if (content.length === 0) return;

      var wrapper = document.createElement('div');
      wrapper.className = 'section-collapsible';

      var btn = document.createElement('button');
      btn.className = 'section-header';
      btn.setAttribute('type', 'button');
      btn.innerHTML = '<span class="section-title">' + h5.textContent + '</span>' +
        '<svg class="section-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">' +
        '<path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>';

      var body = document.createElement('div');
      body.className = 'section-body';

      if (index === 0) {
        btn.classList.add('active');
        body.classList.add('open');
      }

      content.forEach(function(el) { body.appendChild(el); });

      btn.addEventListener('click', function() {
        var isOpen = body.classList.contains('open');
        container.querySelectorAll('.section-body').forEach(function(b) {
          b.classList.remove('open');
        });
        container.querySelectorAll('.section-header').forEach(function(b) {
          b.classList.remove('active');
        });
        if (!isOpen) {
          body.classList.add('open');
          btn.classList.add('active');
          btn.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
      });

      wrapper.appendChild(btn);
      wrapper.appendChild(body);
      h5.replaceWith(wrapper);
    });

    // Reveal the container now that sections are built
    container.classList.remove('has-sections');
    container.classList.add('sections-ready');
  }

  function scanAndInit() {
    hideUnprocessed();
    document.querySelectorAll('.step-content').forEach(initSections);
  }

  setInterval(scanAndInit, 300);
  document.addEventListener('DOMContentLoaded', scanAndInit);
  scanAndInit();
})();
