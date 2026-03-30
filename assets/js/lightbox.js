// Image lightbox for guide step content.
// Intercepts clicks on images inside .step-content, shows them in a
// full-screen modal overlay with zoom animation. Click or ESC to close.
(function(){
  var overlay = document.createElement('div');
  overlay.id = 'lightbox-overlay';
  overlay.innerHTML = '<img id="lightbox-img" />';
  document.body.appendChild(overlay);

  var img = document.getElementById('lightbox-img');

  function close() {
    overlay.classList.remove('active');
    setTimeout(function(){ overlay.style.display = 'none'; }, 200);
  }

  overlay.addEventListener('click', close);

  document.addEventListener('keydown', function(e) {
    if (e.key === 'Escape') close();
  });

  // Delegate click events on images inside step content
  document.addEventListener('click', function(e) {
    var target = e.target;
    // Match img tags inside step content (rendered via inner_html)
    if (target.tagName === 'IMG' && target.closest('.step-content')) {
      e.preventDefault();
      e.stopPropagation();
      // Also prevent the parent <a> from navigating
      var parentLink = target.closest('a');
      if (parentLink) {
        e.preventDefault();
      }
      img.src = target.src;
      img.alt = target.alt || '';
      overlay.style.display = 'flex';
      // Trigger animation on next frame
      requestAnimationFrame(function(){
        overlay.classList.add('active');
      });
    }
  }, true);
})();
