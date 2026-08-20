// Localized file picker (ux-language-consistency).
//
// The native <input type="file"> is rendered by the browser in the
// OS language (e.g. "选择文件" / "未选择任何文件" on a zh-CN system)
// even though the app UI is English. We replace the native control's
// visible chrome with a styled button + filename label that are always
// in the app's language. The hidden input stays in the form and keeps
// participating in validation.
(function () {
  'use strict';

  function enhance(input) {
    if (input.dataset.oaFileEnhanced) return;
    input.dataset.oaFileEnhanced = '1';

    var label = document.createElement('label');
    label.className =
      'oa-file-picker inline-flex items-center gap-2 cursor-pointer';

    var button = document.createElement('span');
    button.className =
      'rounded bg-slate-800 text-white px-3 py-1.5 text-sm hover:bg-slate-700 ' +
      'dark:bg-slate-700 dark:hover:bg-slate-600';
    button.textContent = 'Choose file';

    var status = document.createElement('span');
    // Fixed width keeps the control's total size constant no matter how
    // long the chosen filename is, so picking a file never reflows the
    // surrounding form (no jump/vibration). The full name is available
    // via the title tooltip.
    status.className = 'text-xs text-slate-500 truncate w-36 sm:w-48';
    status.textContent = 'No file chosen';

    // Hide the native control (its own label would be localized).
    input.classList.add('sr-only');

    input.parentNode.insertBefore(label, input);
    label.appendChild(input);
    label.appendChild(button);
    label.appendChild(status);

    input.addEventListener('change', function () {
      var n = input.files ? input.files.length : 0;
      if (n === 0) {
        status.textContent = 'No file chosen';
        status.title = '';
      } else if (n === 1) {
        status.textContent = input.files[0].name;
        status.title = input.files[0].name;
      } else {
        status.textContent = n + ' files chosen';
        status.title = '';
      }
    });
  }

  function scan() {
    document.querySelectorAll('input[type="file"]').forEach(enhance);
  }

  document.addEventListener('DOMContentLoaded', scan);
  // Forms swapped in by HTMX need their file inputs enhanced too.
  document.addEventListener('htmx:afterSwap', scan);
})();
