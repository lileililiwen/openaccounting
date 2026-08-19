// App-owned form validation messages (ux-language-consistency).
//
// Browsers render HTML5 validation tooltips in the OS language
// (e.g. Chinese on a zh-CN system) even when the app UI is English.
// We intercept the `invalid` event and stamp the app's own English
// message onto the field via setCustomValidity, so the tooltip that
// appears is always in the UI language.
//
// The message is cleared on input/change so a corrected field passes
// validation again on the next submit.
(function () {
  'use strict';

  function messageFor(el) {
    var v = el.validity;
    if (v.valueMissing) {
      return el.tagName === 'SELECT' ? 'Please choose an option.' : 'Please fill in this field.';
    }
    if (v.typeMismatch) {
      if (el.type === 'email') return 'Please enter a valid email address.';
      if (el.type === 'url') return 'Please enter a valid URL.';
      return 'Please enter a valid value.';
    }
    if (v.tooShort) {
      return 'Please lengthen this value to ' + el.minLength + ' characters or more.';
    }
    if (v.tooLong) {
      return 'Please shorten this value to ' + el.maxLength + ' characters or less.';
    }
    if (v.badInput) return 'Please enter a valid number.';
    if (v.stepMismatch) return 'Please enter a valid value.';
    if (v.rangeUnderflow) return 'Please choose a value of at least ' + el.min + '.';
    if (v.rangeOverflow) return 'Please choose a value no larger than ' + el.max + '.';
    return 'Please fix this field.';
  }

  document.addEventListener('invalid', function (e) {
    var el = e.target;
    if (!el || !el.form || typeof el.setCustomValidity !== 'function') return;
    if (!el.validity.valid) {
      el.setCustomValidity(messageFor(el));
    }
  }, true);

  // Once the user edits a field, drop the custom message so the
  // field no longer counts as invalid.
  document.addEventListener('input', function (e) {
    var el = e.target;
    if (el && typeof el.setCustomValidity === 'function') {
      el.setCustomValidity('');
    }
  }, true);
})();
