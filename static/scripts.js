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
      console.log(el.children[0].href)
      console.log(currentPage.href)
      if ( currentPage.href == el.children[0].href ) {
        el.children[0].classList.toggle('is-active');
      }
    });
  }
});


var page_index = 0;
var char_id = "SO";
const load_history = () => {

    let req = new XMLHttpRequest();
    req.onreadystatechange = function() {
        if (this.readyState = 4 && this.status == 200) {
            console.log('got history');

            document.getElementById('history').innerHTML = this.responseText;
            document.getElementById("decrement_button").disabled = page_index == 0;
            document.getElementById("current_page").innerHTML = "Games " + (page_index * 100 + 1) + "–" + (page_index * 100 + 100)
        }
    };
    console.log('requesting history');
    req.open('GET', char_id + '/history?offset=' + page_index * 100, true);
    req.send();
}
const increment_page = () => {
    page_index += 1;
    load_history();
}
const decrement_page = () => {
    if (page_index > 0) {
        page_index -= 1;
        load_history()
    }
}
