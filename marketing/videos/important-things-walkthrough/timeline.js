(() => {
  let started = false;

  window.startFilm = () => {
    if (started) return;
    started = true;
    document.body.classList.add('playing');
  };

  window.resetFilm = () => {
    document.body.classList.remove('playing');
    void document.body.offsetWidth;
    started = false;
  };

  if (new URLSearchParams(window.location.search).has('autoplay')) {
    requestAnimationFrame(() => requestAnimationFrame(window.startFilm));
  }
})();
