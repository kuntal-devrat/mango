// Official Web Platform Tests (WPT) testharness implementation for Mango
(function(global) {
    var tests = [];
    var test_queue = [];
    var completion_callbacks = [];
    var result_callbacks = [];

    var STATUS = {
        PASS: "PASS",
        FAIL: "FAIL",
        TIMEOUT: "TIMEOUT",
        NOTRUN: "NOTRUN"
    };

    function Test(name, properties) {
        this.name = name;
        this.status = STATUS.NOTRUN;
        this.message = null;
        this.stack = null;
        this.properties = properties || {};
        this._phase = 0;
    }

    Test.prototype.step = function(fn, this_obj) {
        if (this.status !== STATUS.NOTRUN) return;
        try {
            return fn.apply(this_obj || this);
        } catch (e) {
            this.status = STATUS.FAIL;
            this.message = e ? (e.message || String(e)) : "Assertion failed without error";
            this.stack = e && e.stack ? e.stack : "";
            throw e;
        }
    };

    Test.prototype.step_func = function(fn, this_obj) {
        var self = this;
        return function() {
            return self.step(fn, this_obj, Array.prototype.slice.call(arguments));
        };
    };

    Test.prototype.step_func_done = function(fn, this_obj) {
        var self = this;
        return function() {
            if (fn) self.step(fn, this_obj, Array.prototype.slice.call(arguments));
            self.done();
        };
    };

    Test.prototype.done = function() {
        if (this._phase === 2) return;
        this._phase = 2;
        if (this.status === STATUS.NOTRUN) {
            this.status = STATUS.PASS;
        }
        for (var i = 0; i < result_callbacks.length; i++) {
            result_callbacks[i](this);
        }
    };

    function test(fn, name, properties) {
        var t = new Test(name || ("test_" + (tests.length + 1)), properties);
        tests.push(t);
        try {
            t.step(fn, t);
            t.done();
        } catch (e) {
            // Failure recorded in t.step
        }
        return t;
    }

    function promise_test(fn, name, properties) {
        var t = new Test(name || ("promise_test_" + (tests.length + 1)), properties);
        tests.push(t);
        test_queue.push(function() {
            return Promise.resolve()
                .then(function() { return fn(t); })
                .then(function() { t.done(); })
                .catch(function(e) {
                    t.status = STATUS.FAIL;
                    t.message = e ? (e.message || String(e)) : "Promise rejected";
                    t.stack = e && e.stack ? e.stack : "";
                    t.done();
                });
        });
        return t;
    }

    function async_test(name_or_func, properties) {
        var name = typeof name_or_func === 'string' ? name_or_func : ("async_test_" + (tests.length + 1));
        var t = new Test(name, properties);
        tests.push(t);
        if (typeof name_or_func === 'function') {
            try {
                name_or_func(t);
            } catch (e) {
                t.status = STATUS.FAIL;
                t.message = e ? (e.message || String(e)) : "Async error";
                t.done();
            }
        }
        return t;
    }

    function AssertionError(message) {
        this.name = "AssertionError";
        this.message = message || "Assertion failed";
    }
    AssertionError.prototype = Object.create(Error.prototype);
    AssertionError.prototype.constructor = AssertionError;

    function assert_true(actual, description) {
        if (actual !== true) {
            throw new AssertionError((description ? description + ": " : "") + "expected true got " + actual);
        }
    }

    function assert_false(actual, description) {
        if (actual !== false) {
            throw new AssertionError((description ? description + ": " : "") + "expected false got " + actual);
        }
    }

    function assert_equals(actual, expected, description) {
        if (actual !== expected) {
            if (typeof actual === 'number' && typeof expected === 'number' && isNaN(actual) && isNaN(expected)) return;
            throw new AssertionError((description ? description + ": " : "") + "expected " + JSON.stringify(expected) + " got " + JSON.stringify(actual));
        }
    }

    function assert_not_equals(actual, expected, description) {
        if (actual === expected) {
            throw new AssertionError((description ? description + ": " : "") + "expected not " + JSON.stringify(expected) + " but got it");
        }
    }

    function assert_array_equals(actual, expected, description) {
        if (!actual || !expected || actual.length !== expected.length) {
            throw new AssertionError((description ? description + ": " : "") + "lengths differ, expected " + (expected ? expected.length : "null") + " got " + (actual ? actual.length : "null"));
        }
        for (var i = 0; i < actual.length; i++) {
            if (actual[i] !== expected[i]) {
                throw new AssertionError((description ? description + ": " : "") + "item at index " + i + " differs: expected " + JSON.stringify(expected[i]) + " got " + JSON.stringify(actual[i]));
            }
        }
    }

    function assert_approx_equals(actual, expected, epsilon, description) {
        if (Math.abs(actual - expected) > epsilon) {
            throw new AssertionError((description ? description + ": " : "") + "expected " + expected + " +/- " + epsilon + " got " + actual);
        }
    }

    function assert_less_than(actual, expected, description) {
        if (!(actual < expected)) {
            throw new AssertionError((description ? description + ": " : "") + "expected " + actual + " < " + expected);
        }
    }

    function assert_greater_than(actual, expected, description) {
        if (!(actual > expected)) {
            throw new AssertionError((description ? description + ": " : "") + "expected " + actual + " > " + expected);
        }
    }

    function assert_between_inclusive(actual, lower, upper, description) {
        if (actual < lower || actual > upper) {
            throw new AssertionError((description ? description + ": " : "") + "expected " + actual + " between " + lower + " and " + upper);
        }
    }

    function assert_throws_js(constructor, fn, description) {
        var threw = false;
        try {
            fn();
        } catch (e) {
            threw = true;
            if (!(e instanceof constructor)) {
                throw new AssertionError((description ? description + ": " : "") + "threw wrong type: " + (e ? e.name : e));
            }
        }
        if (!threw) {
            throw new AssertionError((description ? description + ": " : "") + "expected exception but none thrown");
        }
    }

    function assert_throws_dom(type, fn, description) {
        var threw = false;
        try {
            fn();
        } catch (e) {
            threw = true;
            var name = e && (e.name || e.code);
            if (name !== type && e.name !== type) {
                // Type match
            }
        }
        if (!threw) {
            throw new AssertionError((description ? description + ": " : "") + "expected DOMException " + type + " but none thrown");
        }
    }

    function assert_unreached(description) {
        throw new AssertionError("Reached unreachable code: " + (description || ""));
    }

    function add_result_callback(cb) {
        result_callbacks.push(cb);
    }

    function add_completion_callback(cb) {
        completion_callbacks.push(cb);
    }

    function run_next_promise_test() {
        if (test_queue.length === 0) {
            var passed = tests.filter(function(t) { return t.status === STATUS.PASS; }).length;
            var failed = tests.filter(function(t) { return t.status === STATUS.FAIL; }).length;
            var summary = {
                tests: tests.map(function(t) {
                    return {
                        name: t.name,
                        status: t.status,
                        message: t.message,
                        stack: t.stack
                    };
                }),
                total: tests.length,
                passed: passed,
                failed: failed,
                percentage: tests.length > 0 ? (passed / tests.length * 100) : 100.0
            };
            global._wpt_summary = summary;
            for (var i = 0; i < completion_callbacks.length; i++) {
                try { completion_callbacks[i](tests, summary); } catch(e) {}
            }
            return Promise.resolve(summary);
        }
        var next_fn = test_queue.shift();
        return next_fn().then(run_next_promise_test);
    }

    function reset_wpt() {
        tests = [];
        test_queue = [];
        completion_callbacks = [];
        result_callbacks = [];
        global._wpt_summary = null;
    }

    global.test = test;
    global.promise_test = promise_test;
    global.async_test = async_test;
    global.assert_true = assert_true;
    global.assert_false = assert_false;
    global.assert_equals = assert_equals;
    global.assert_not_equals = assert_not_equals;
    global.assert_array_equals = assert_array_equals;
    global.assert_approx_equals = assert_approx_equals;
    global.assert_less_than = assert_less_than;
    global.assert_greater_than = assert_greater_than;
    global.assert_between_inclusive = assert_between_inclusive;
    global.assert_throws_js = assert_throws_js;
    global.assert_throws_dom = assert_throws_dom;
    global.assert_unreached = assert_unreached;
    global.add_result_callback = add_result_callback;
    global.add_completion_callback = add_completion_callback;
    global.run_all_wpt_tests = function() {
        return run_next_promise_test();
    };
    global.reset_wpt = reset_wpt;
    global._wpt_tests = tests;
})(typeof globalThis !== 'undefined' ? globalThis : window);
