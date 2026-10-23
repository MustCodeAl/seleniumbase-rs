// Recorder Mode: SHIFT+ESC ends a recording made with `mkrec --ee`.
//
// A port of `_get_rec_shift_esc_script` from SeleniumBase 4.56.2
// (seleniumbase/fixtures/base_case.py, MIT licence). Python assigns
// `document.onkeydown`; this adds a listener once per document instead, so a
// page that sets its own `onkeydown` neither loses it nor removes this one.
// Running it again on the same document does nothing, which is what lets the
// watch loop arm it on every tick.
(function () {
    if (document.__sbEscEndHooked) return;
    document.__sbEscEndHooked = true;
    document.addEventListener('keydown', function (evt) {
        evt = evt || window.event;
        var isEscape = false;
        var last_key;
        if ("key" in evt) {
            isEscape = (evt.key === "Escape" || evt.key === "Esc");
            last_key = evt.key;
        } else {
            isEscape = (evt.keyCode === 27);
            last_key = evt.keyCode;
            if (last_key === 16) {
                last_key = "Shift";
            }
        }
        if (isEscape && document.sb_last_key === "Shift") {
            document.sb_esc_end = "yes";
        }
        document.sb_last_key = last_key;
    }, true);
})();
