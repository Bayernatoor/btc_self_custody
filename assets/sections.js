// Collapsible sections within step content.
// Turns <h5> tags inside .step-content into clickable section headers
// that expand/collapse the content between them.
(function(){
  function initSections() {
    document.querySelectorAll('.step-content').forEach(function(container) {
      // Skip if already processed
      if (container.dataset.sectionsInit) return;
      container.dataset.sectionsInit = 'true';

      var headings = container.querySelectorAll('h5');
      if (headings.length === 0) return;

      headings.forEach(function(h5, index) {
        // Collect all siblings between this h5 and the next h5 (or end)
        var content = [];
        var sibling = h5.nextElementSibling;
        while (sibling && sibling.tagName !== 'H5') {
          content.push(sibling);
          sibling = sibling.nextElementSibling;
        }

        // Create wrapper
        var wrapper = document.createElement('div');
        wrapper.className = 'section-collapsible';

        // Create header button
        var btn = document.createElement('button');
        btn.className = 'section-header';
        btn.setAttribute('type', 'button');
        btn.innerHTML = '<span class="section-title">' + h5.textContent + '</span>' +
          '<svg class="section-chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">' +
          '<path stroke-linecap="round" stroke-linejoin="round" d="M19 9l-7 7-7-7"/></svg>';

        // Create content container
        var body = document.createElement('div');
        body.className = 'section-body';
        // First section open by default
        if (index === 0) {
          btn.classList.add('active');
          body.classList.add('open');
        }

        // Move content into body
        content.forEach(function(el) { body.appendChild(el); });

        // Toggle handler
        btn.addEventListener('click', function() {
          var isOpen = body.classList.contains('open');
          // Close all sections in this container
          wrapper.parentNode.querySelectorAll('.section-body').forEach(function(b) {
            b.classList.remove('open');
          });
          wrapper.parentNode.querySelectorAll('.section-header').forEach(function(b) {
            b.classList.remove('active');
          });
          // Open this one if it was closed
          if (!isOpen) {
            body.classList.add('open');
            btn.classList.add('active');
            btn.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
          }
        });

        // Replace h5 with the wrapper
        wrapper.appendChild(btn);
        wrapper.appendChild(body);
        h5.replaceWith(wrapper);
      });
    });
  }

  // Run on load and on every DOM change (for Leptos hydration/navigation)
  var observer = new MutationObserver(function() {
    requestAnimationFrame(initSections);
  });
  observer.observe(document.body, { childList: true, subtree: true });
  document.addEventListener('DOMContentLoaded', initSections);
  // Also run immediately in case DOM is already ready
  initSections();
})();
