document.addEventListener('DOMContentLoaded', () => {
  const $navbarBurgers = Array.prototype.slice.call(document.querySelectorAll('.navbar-burger'), 0);
  if ($navbarBurgers.length > 0) {
    $navbarBurgers.forEach( el => {
      el.addEventListener('click', () => {
        const target = el.dataset.target;
        const $target = document.getElementById(target);
        el.classList.toggle('is-active');
        $target.classList.toggle('is-active');
      });
    });
  }

  const $charTabs = Array.prototype.slice.call(document.querySelectorAll('.charTab'));
  var currentPage = window.location || window.document.location;
  if ($charTabs.length > 0) {
    $charTabs.forEach ( el => {
      if ( currentPage.href == el.children[0].href ) {
        el.classList.toggle('is-active');
      }
    });
  }
});