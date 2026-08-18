// CSRF helper for HTMX (`s1-csrf-protection`).
//
// Reads the session-bound token from `<meta name="csrf-token">`
// (injected by the server's CSRF middleware) and attaches it to
// every HTMX request via the `X-CSRF-Token` header, matching the
// form-field counterpart the middleware accepts on urlencoded
// POSTs.

(function () {
  function readToken() {
    var meta = document.querySelector('meta[name="csrf-token"]');
    return meta ? meta.getAttribute('content') : null;
  }
  document.addEventListener('htmx:configRequest', function (evt) {
    var token = readToken();
    if (token) {
      evt.detail.headers['X-CSRF-Token'] = token;
    }
  });
})();