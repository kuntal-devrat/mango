// ── 21. WPT test_driver automation shim ────────────────────────────────────
(function() {
    if (typeof globalThis.test_driver !== 'undefined') return;

    var test_driver = {
        click: function(element) {
            return new Promise(function(resolve, reject) {
                if (!element) return reject(new Error("test_driver.click requires an element"));
                try {
                    if (typeof element.focus === 'function') element.focus();
                    var rect = (element.getBoundingClientRect && element.getBoundingClientRect()) || { left: 0, top: 0, width: 10, height: 10 };
                    var clientX = rect.left + rect.width / 2;
                    var clientY = rect.top + rect.height / 2;

                    var pointerdown = new MouseEvent('pointerdown', { bubbles: true, cancelable: true, clientX: clientX, clientY: clientY });
                    var mousedown = new MouseEvent('mousedown', { bubbles: true, cancelable: true, clientX: clientX, clientY: clientY });
                    var pointerup = new MouseEvent('pointerup', { bubbles: true, cancelable: true, clientX: clientX, clientY: clientY });
                    var mouseup = new MouseEvent('mouseup', { bubbles: true, cancelable: true, clientX: clientX, clientY: clientY });
                    var click = new MouseEvent('click', { bubbles: true, cancelable: true, clientX: clientX, clientY: clientY });

                    element.dispatchEvent(pointerdown);
                    element.dispatchEvent(mousedown);
                    element.dispatchEvent(pointerup);
                    element.dispatchEvent(mouseup);
                    element.dispatchEvent(click);
                    resolve();
                } catch(e) {
                    reject(e);
                }
            });
        },

        send_keys: function(element, keys) {
            return new Promise(function(resolve, reject) {
                if (!element) return reject(new Error("test_driver.send_keys requires an element"));
                try {
                    if (typeof element.focus === 'function') element.focus();
                    var keyStr = String(keys);
                    for (var i = 0; i < keyStr.length; i++) {
                        var char = keyStr[i];
                        var kd = new KeyboardEvent('keydown', { key: char, bubbles: true, cancelable: true });
                        var kp = new KeyboardEvent('keypress', { key: char, bubbles: true, cancelable: true });
                        element.dispatchEvent(kd);
                        element.dispatchEvent(kp);
                        if ('value' in element && typeof element.value === 'string') {
                            element.value += char;
                            var inp = new Event('input', { bubbles: true, cancelable: true });
                            element.dispatchEvent(inp);
                        }
                        var ku = new KeyboardEvent('keyup', { key: char, bubbles: true, cancelable: true });
                        element.dispatchEvent(ku);
                    }
                    resolve();
                } catch(e) {
                    reject(e);
                }
            });
        },

        send_wheel: function(element, deltaX, deltaY) {
            return new Promise(function(resolve, reject) {
                try {
                    var target = element || (typeof document !== 'undefined' && (document.body || document.documentElement));
                    var evt = new WheelEvent('wheel', {
                        bubbles: true,
                        cancelable: true,
                        deltaX: deltaX || 0,
                        deltaY: deltaY || 0,
                        deltaMode: 0
                    });
                    if (target && target.dispatchEvent) target.dispatchEvent(evt);
                    resolve();
                } catch(e) {
                    reject(e);
                }
            });
        },

        set_permission: function(descriptor, state) {
            return Promise.resolve();
        },

        delete_all_cookies: function() {
            return new Promise(function(resolve) {
                if (typeof document !== 'undefined') {
                    var cookies = document.cookie ? document.cookie.split(';') : [];
                    for (var i = 0; i < cookies.length; i++) {
                        var eq = cookies[i].indexOf('=');
                        var name = eq > -1 ? cookies[i].substring(0, eq).trim() : cookies[i].trim();
                        document.cookie = name + '=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/';
                    }
                }
                resolve();
            });
        },

        action_sequence: function(actions) {
            return Promise.resolve();
        },

        bless: function(intent, action) {
            if (typeof action === 'function') return action();
            return Promise.resolve();
        }
    };

    globalThis.test_driver = test_driver;
    globalThis.test_driver_internal = test_driver;
})();
